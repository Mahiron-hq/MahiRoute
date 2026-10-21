// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

//! An authenticated ADB connection over wireless debugging and the shell service on top of it.
//!
//! After pairing, `adbd` accepts clients whose TLS certificate carries a registered key, so the
//! classic RSA challenge is replaced by a TLS upgrade: connect, announce, switch to TLS, and
//! the device answers with its banner.

use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};

use boring::ssl::SslStream;

use crate::error::AdbError;
use crate::key::AdbKey;
use crate::tls;
use crate::wire::{
    A_AUTH, A_CLSE, A_CNXN, A_OKAY, A_OPEN, A_STLS, A_WRTE, MAX_PAYLOAD, Message,
    VERSION_SKIP_CHECKSUM,
};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const IO_TIMEOUT: Duration = Duration::from_secs(10);
const LOCAL_STREAM_ID: u32 = 1;

/// A connected, TLS-protected session with `adbd`.
pub struct Session {
    tls: SslStream<TcpStream>,
}

impl Session {
    /// Connects to `address` and completes the TLS upgrade with `key`.
    ///
    /// # Errors
    ///
    /// Returns [`AdbError::Protocol`] when the device does not offer TLS (wireless debugging is
    /// off or the connection is a classic one), [`AdbError::Tls`] when the device refuses this
    /// key (the app is not paired) and [`AdbError::Io`] for network failures.
    pub fn connect(address: SocketAddr, key: &AdbKey) -> Result<Self, AdbError> {
        let mut stream = TcpStream::connect_timeout(&address, CONNECT_TIMEOUT)?;
        stream.set_nodelay(true)?;
        stream.set_read_timeout(Some(IO_TIMEOUT))?;
        stream.set_write_timeout(Some(IO_TIMEOUT))?;

        let announce = Message::with_data(
            A_CNXN,
            VERSION_SKIP_CHECKSUM,
            u32::try_from(MAX_PAYLOAD).unwrap_or(u32::MAX),
            b"host::\0".to_vec(),
        );
        announce.write_to(&mut stream)?;
        let offer = Message::read_from(&mut stream)?;
        match offer.command {
            A_STLS => {}
            A_AUTH => {
                return Err(AdbError::Protocol(
                    "the device asks for classic authentication; turn on wireless debugging"
                        .to_owned(),
                ));
            }
            other => {
                return Err(AdbError::Protocol(format!(
                    "unexpected answer {other:#010x} to the connection request"
                )));
            }
        }
        Message::new(A_STLS, offer.arg0, 0).write_to(&mut stream)?;

        let mut tls = tls::connect(&tls::client_connector(key)?, stream)?;
        let banner = Message::read_from(&mut tls).map_err(refused)?;
        if banner.command != A_CNXN {
            return Err(AdbError::Protocol(format!(
                "expected a banner, got {:#010x}",
                banner.command
            )));
        }
        Ok(Self { tls })
    }

    /// Runs `command` through the device's shell and returns everything it printed.
    ///
    /// # Errors
    ///
    /// Returns [`AdbError::Command`] when the command does not finish within `timeout` and
    /// [`AdbError::Io`] or [`AdbError::Protocol`] when the connection misbehaves.
    pub fn shell(&mut self, command: &str, timeout: Duration) -> Result<Vec<u8>, AdbError> {
        let deadline = Instant::now() + timeout;
        let mut service = format!("shell:{command}").into_bytes();
        service.push(0);
        Message::with_data(A_OPEN, LOCAL_STREAM_ID, 0, service).write_to(&mut self.tls)?;

        let mut remote_id = 0u32;
        let mut output = Vec::new();
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(AdbError::Command("the shell command timed out".to_owned()));
            }
            self.tls.get_ref().set_read_timeout(Some(remaining))?;
            let message = match Message::read_from(&mut self.tls) {
                Ok(message) => message,
                Err(AdbError::Io(error))
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    return Err(AdbError::Command("the shell command timed out".to_owned()));
                }
                Err(error) => return Err(error),
            };
            match message.command {
                A_OKAY => remote_id = message.arg0,
                A_WRTE => {
                    output.extend_from_slice(&message.data);
                    Message::new(A_OKAY, LOCAL_STREAM_ID, message.arg0).write_to(&mut self.tls)?;
                }
                A_CLSE => return Ok(output),
                _ => {}
            }
            if remote_id == 0 && message.command == A_CLSE {
                return Err(AdbError::Command(
                    "the device refused to open a shell".to_owned(),
                ));
            }
        }
    }
}

/// TLS 1.3 reports a rejected client certificate only when the first data is read, as an alert or
/// a hang-up, so such failures at that point mean "this key is not paired".
fn refused(error: AdbError) -> AdbError {
    match error {
        AdbError::Io(io_error)
            if io_error.to_string().to_ascii_uppercase().contains("ALERT")
                || matches!(
                    io_error.kind(),
                    std::io::ErrorKind::UnexpectedEof | std::io::ErrorKind::ConnectionReset
                ) =>
        {
            AdbError::Tls(format!("the device refused this client: {io_error}"))
        }
        other => other,
    }
}

/// Connects, runs one shell command and returns its output as text.
///
/// # Errors
///
/// Returns the errors of [`Session::connect`] and [`Session::shell`].
pub fn run_shell(
    address: SocketAddr,
    key: &AdbKey,
    command: &str,
    timeout: Duration,
) -> Result<String, AdbError> {
    let mut session = Session::connect(address, key)?;
    let output = session.shell(command, timeout)?;
    Ok(String::from_utf8_lossy(&output).into_owned())
}
