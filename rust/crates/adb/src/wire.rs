// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

//! The ADB message framing: a 24-byte little-endian header followed by an optional payload.

use std::io::{Read, Write};

use crate::error::AdbError;

/// `CNXN`: connection request and answer.
pub const A_CNXN: u32 = 0x4e58_4e43;
/// `AUTH`: classic RSA authentication step.
pub const A_AUTH: u32 = 0x4854_5541;
/// `OPEN`: opens a stream to a service.
pub const A_OPEN: u32 = 0x4e45_504f;
/// `OKAY`: acknowledges a stream or a write.
pub const A_OKAY: u32 = 0x5941_4b4f;
/// `CLSE`: closes a stream.
pub const A_CLSE: u32 = 0x4553_4c43;
/// `WRTE`: stream data.
pub const A_WRTE: u32 = 0x4554_5257;
/// `STLS`: switch the connection to TLS.
pub const A_STLS: u32 = 0x534c_5453;

/// Protocol version that lets both sides skip payload checksums.
pub const VERSION_SKIP_CHECKSUM: u32 = 0x0100_0001;
/// Largest payload this client accepts or announces.
pub const MAX_PAYLOAD: usize = 1 << 20;

const HEADER_LEN: usize = 24;

/// One ADB message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// Command word, one of the `A_*` constants.
    pub command: u32,
    /// First argument; its meaning depends on the command.
    pub arg0: u32,
    /// Second argument; its meaning depends on the command.
    pub arg1: u32,
    /// Payload bytes.
    pub data: Vec<u8>,
}

impl Message {
    /// Builds a message without payload.
    #[must_use]
    pub fn new(command: u32, arg0: u32, arg1: u32) -> Self {
        Self {
            command,
            arg0,
            arg1,
            data: Vec::new(),
        }
    }

    /// Builds a message with payload.
    #[must_use]
    pub fn with_data(command: u32, arg0: u32, arg1: u32, data: Vec<u8>) -> Self {
        Self {
            command,
            arg0,
            arg1,
            data,
        }
    }

    /// Serializes the header and payload.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let length = u32::try_from(self.data.len()).unwrap_or(u32::MAX);
        let checksum = self
            .data
            .iter()
            .fold(0u32, |sum, byte| sum.wrapping_add(u32::from(*byte)));
        let mut out = Vec::with_capacity(HEADER_LEN + self.data.len());
        for word in [
            self.command,
            self.arg0,
            self.arg1,
            length,
            checksum,
            self.command ^ 0xffff_ffff,
        ] {
            out.extend_from_slice(&word.to_le_bytes());
        }
        out.extend_from_slice(&self.data);
        out
    }

    /// Writes the message to `writer` in one call and flushes it.
    ///
    /// # Errors
    ///
    /// Returns [`AdbError::Io`] when the write fails.
    pub fn write_to(&self, writer: &mut impl Write) -> Result<(), AdbError> {
        writer.write_all(&self.encode())?;
        writer.flush()?;
        Ok(())
    }

    /// Reads one message, refusing payloads above [`MAX_PAYLOAD`].
    ///
    /// # Errors
    ///
    /// Returns [`AdbError::Io`] when the stream ends early and [`AdbError::Protocol`] when the
    /// header magic is wrong or the payload is too large.
    pub fn read_from(reader: &mut impl Read) -> Result<Self, AdbError> {
        let mut header = [0u8; HEADER_LEN];
        reader.read_exact(&mut header)?;
        let word = |index: usize| {
            let start = index * 4;
            u32::from_le_bytes([
                header[start],
                header[start + 1],
                header[start + 2],
                header[start + 3],
            ])
        };
        let (command, arg0, arg1, length, magic) =
            (word(0), word(1), word(2), word(3) as usize, word(5));
        if magic != command ^ 0xffff_ffff {
            return Err(AdbError::Protocol(format!(
                "bad magic for command {command:#010x}"
            )));
        }
        if length > MAX_PAYLOAD {
            return Err(AdbError::Protocol(format!(
                "payload of {length} bytes is too large"
            )));
        }
        let mut data = vec![0u8; length];
        reader.read_exact(&mut data)?;
        Ok(Self {
            command,
            arg0,
            arg1,
            data,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::io::Cursor;

    #[test]
    fn header_layout_matches_the_protocol() {
        let message = Message::with_data(A_OPEN, 7, 0, b"shell:id\0".to_vec());
        let bytes = message.encode();
        assert_eq!(&bytes[0..4], b"OPEN");
        assert_eq!(
            u32::from_le_bytes(bytes[4..8].try_into().expect("4 bytes")),
            7
        );
        assert_eq!(
            u32::from_le_bytes(bytes[12..16].try_into().expect("4 bytes")),
            9
        );
        assert_eq!(&bytes[24..], b"shell:id\0");
        assert_eq!(
            u32::from_le_bytes(bytes[20..24].try_into().expect("4 bytes")),
            A_OPEN ^ 0xffff_ffff
        );
    }

    #[test]
    fn rejects_a_corrupted_magic() {
        let mut bytes = Message::new(A_OKAY, 1, 2).encode();
        bytes[20] ^= 0xff;
        assert!(matches!(
            Message::read_from(&mut Cursor::new(bytes)),
            Err(AdbError::Protocol(_))
        ));
    }

    #[test]
    fn rejects_oversized_payloads_before_allocating_them() {
        let mut bytes = Message::new(A_WRTE, 1, 2).encode();
        bytes[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(
            Message::read_from(&mut Cursor::new(bytes)),
            Err(AdbError::Protocol(_))
        ));
    }

    #[test]
    fn truncated_streams_are_io_errors() {
        let bytes = Message::with_data(A_WRTE, 1, 2, vec![0; 10]).encode();
        let cut = bytes[..bytes.len() - 3].to_vec();
        assert!(matches!(
            Message::read_from(&mut Cursor::new(cut)),
            Err(AdbError::Io(_))
        ));
    }

    proptest! {
        #[test]
        fn messages_round_trip(
            command in any::<u32>(),
            arg0 in any::<u32>(),
            arg1 in any::<u32>(),
            data in proptest::collection::vec(any::<u8>(), 0..512),
        ) {
            let message = Message::with_data(command, arg0, arg1, data);
            let decoded = Message::read_from(&mut Cursor::new(message.encode())).expect("decode");
            prop_assert_eq!(decoded, message);
        }

        #[test]
        fn reading_garbage_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..200)) {
            let _ = Message::read_from(&mut Cursor::new(bytes));
        }
    }
}
