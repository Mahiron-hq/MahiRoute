// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

//! Transport abstractions and the loopback TCP implementation.

use std::io::{self, BufRead, BufReader, Write};
use std::net::{Ipv4Addr, Shutdown, SocketAddr, TcpStream};
use std::time::Duration;

use mahiroute_core::protocol::MAX_LINE_BYTES;

use crate::core::HelperTarget;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);

/// Write half of a helper connection.
pub trait Transport: Send {
    /// Sends one complete line, including its terminator.
    ///
    /// # Errors
    ///
    /// Returns the I/O error that made the write fail.
    fn send_line(&mut self, line: &str) -> io::Result<()>;

    /// Closes the connection, unblocking the reader.
    fn close(&mut self);
}

/// Read half of a helper connection.
pub trait LineSource: Send {
    /// Blocks until a line (without terminator) arrives; `None` means the peer hung up.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the connection fails or a line is malformed.
    fn next_line(&mut self) -> io::Result<Option<String>>;
}

/// Opens connections to the helper.
pub trait Connector: Send + Sync + 'static {
    /// Connects to `target`.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the helper is unreachable.
    fn connect(
        &self,
        target: &HelperTarget,
    ) -> io::Result<(Box<dyn Transport>, Box<dyn LineSource>)>;
}

/// Connects to the helper over loopback TCP.
#[derive(Debug, Default, Clone, Copy)]
pub struct TcpConnector;

struct TcpTransport(TcpStream);

struct TcpLines(BufReader<TcpStream>);

impl Connector for TcpConnector {
    fn connect(
        &self,
        target: &HelperTarget,
    ) -> io::Result<(Box<dyn Transport>, Box<dyn LineSource>)> {
        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, target.port));
        let stream = TcpStream::connect_timeout(&address, CONNECT_TIMEOUT)?;
        stream.set_nodelay(true)?;
        stream.set_write_timeout(Some(WRITE_TIMEOUT))?;
        let reader = stream.try_clone()?;
        Ok((
            Box::new(TcpTransport(stream)),
            Box::new(TcpLines(BufReader::new(reader))),
        ))
    }
}

impl Transport for TcpTransport {
    fn send_line(&mut self, line: &str) -> io::Result<()> {
        self.0.write_all(line.as_bytes())?;
        self.0.flush()
    }

    fn close(&mut self) {
        // The peer may already be gone; there is nothing to do about a failed shutdown.
        let _ = self.0.shutdown(Shutdown::Both);
    }
}

impl LineSource for TcpLines {
    fn next_line(&mut self) -> io::Result<Option<String>> {
        read_bounded_line(&mut self.0)
    }
}

/// Reads one `\n`-terminated line, refusing lines longer than [`MAX_LINE_BYTES`].
///
/// # Errors
///
/// Returns [`io::ErrorKind::InvalidData`] for an oversized or non-UTF-8 line and any error of
/// the underlying reader.
pub fn read_bounded_line(reader: &mut impl BufRead) -> io::Result<Option<String>> {
    let mut line = Vec::new();
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return if line.is_empty() {
                Ok(None)
            } else {
                finish_line(line).map(Some)
            };
        }
        if let Some(end) = available.iter().position(|byte| *byte == b'\n') {
            line.extend_from_slice(&available[..end]);
            reader.consume(end + 1);
            return finish_line(line).map(Some);
        }
        let length = available.len();
        line.extend_from_slice(available);
        reader.consume(length);
        if line.len() > MAX_LINE_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "line exceeds the protocol limit",
            ));
        }
    }
}

fn finish_line(mut bytes: Vec<u8>) -> io::Result<String> {
    if bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
    String::from_utf8(bytes).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn reads_lines_and_signals_end_of_stream() {
        let mut input = Cursor::new(b"one\ntwo\r\nlast".to_vec());
        assert_eq!(
            read_bounded_line(&mut input).expect("read"),
            Some("one".to_owned())
        );
        assert_eq!(
            read_bounded_line(&mut input).expect("read"),
            Some("two".to_owned())
        );
        assert_eq!(
            read_bounded_line(&mut input).expect("read"),
            Some("last".to_owned())
        );
        assert_eq!(read_bounded_line(&mut input).expect("read"), None);
    }

    #[test]
    fn rejects_oversized_lines() {
        let mut input = Cursor::new(vec![b'x'; MAX_LINE_BYTES + 10]);
        let error = read_bounded_line(&mut input).expect_err("too long");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn rejects_invalid_utf8() {
        let mut input = Cursor::new(vec![0xff, 0xfe, b'\n']);
        assert_eq!(
            read_bounded_line(&mut input).expect_err("not utf-8").kind(),
            io::ErrorKind::InvalidData
        );
    }
}
