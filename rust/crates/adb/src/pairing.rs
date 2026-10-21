// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

//! Wireless-debugging pairing: proves knowledge of the six-digit code and registers this app's
//! public key with `adbd`, which afterwards accepts its TLS connections.
//!
//! The protocol runs over TLS 1.3. The code, extended with keying material exported from that
//! TLS session, is the SPAKE2 password; the shared secret derives an AES-128-GCM key that
//! protects the exchange of "peer info" records, of which the client's one carries its key.

use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use boring::symm::{Cipher, decrypt_aead, encrypt_aead};
use hkdf::Hkdf;
use sha2::Sha256;

use crate::error::AdbError;
use crate::key::AdbKey;
use crate::spake2::{Role, Spake2};
use crate::tls;

/// Label of the keying material exported from the TLS session, including its trailing NUL.
pub const EXPORTED_KEY_LABEL: &str = "adb-label\0";
/// Length of the exported keying material.
pub const EXPORTED_KEY_LEN: usize = 64;
/// SPAKE2 identity of the pairing client, including its trailing NUL.
pub const CLIENT_NAME: &[u8] = b"adb pair client\0";
/// SPAKE2 identity of the pairing server, including its trailing NUL.
pub const SERVER_NAME: &[u8] = b"adb pair server\0";
/// Size of a peer-info record before encryption.
pub const PEER_INFO_LEN: usize = 8192;
/// Peer-info type announcing an ADB RSA public key.
pub const PEER_INFO_RSA_PUBLIC_KEY: u8 = 0;
/// Largest packet payload either side may send.
pub const MAX_PAYLOAD: usize = 2 * PEER_INFO_LEN;

const PACKET_VERSION: u8 = 1;
const HEADER_LEN: usize = 6;
const HKDF_INFO: &[u8] = b"adb pairing_auth aes-128-gcm key";
const TAG_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const TIMEOUT: Duration = Duration::from_secs(10);

/// The two packet kinds of the pairing protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketKind {
    /// A SPAKE2 message.
    Spake2Message,
    /// An encrypted peer-info record.
    PeerInfo,
}

impl PacketKind {
    fn wire(self) -> u8 {
        match self {
            Self::Spake2Message => 0,
            Self::PeerInfo => 1,
        }
    }

    fn from_wire(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Spake2Message),
            1 => Some(Self::PeerInfo),
            _ => None,
        }
    }
}

/// Writes one packet: version, kind, big-endian payload length, payload.
///
/// # Errors
///
/// Returns [`AdbError::Io`] when the write fails and [`AdbError::Protocol`] when the payload
/// exceeds [`MAX_PAYLOAD`].
pub fn write_packet(
    writer: &mut impl Write,
    kind: PacketKind,
    payload: &[u8],
) -> Result<(), AdbError> {
    let length = u32::try_from(payload.len())
        .ok()
        .filter(|_| payload.len() <= MAX_PAYLOAD)
        .ok_or_else(|| AdbError::Protocol("pairing payload too large".to_owned()))?;
    let mut packet = Vec::with_capacity(HEADER_LEN + payload.len());
    packet.push(PACKET_VERSION);
    packet.push(kind.wire());
    packet.extend_from_slice(&length.to_be_bytes());
    packet.extend_from_slice(payload);
    writer.write_all(&packet)?;
    writer.flush()?;
    Ok(())
}

/// Reads one packet.
///
/// # Errors
///
/// Returns [`AdbError::Io`] when the stream ends early and [`AdbError::Protocol`] when the
/// header is invalid.
pub fn read_packet(reader: &mut impl Read) -> Result<(PacketKind, Vec<u8>), AdbError> {
    let mut header = [0u8; HEADER_LEN];
    reader.read_exact(&mut header)?;
    if header[0] != PACKET_VERSION {
        return Err(AdbError::Protocol(format!(
            "unsupported pairing version {}",
            header[0]
        )));
    }
    let kind = PacketKind::from_wire(header[1])
        .ok_or_else(|| AdbError::Protocol(format!("unknown pairing packet type {}", header[1])))?;
    let length = u32::from_be_bytes([header[2], header[3], header[4], header[5]]) as usize;
    if length == 0 || length > MAX_PAYLOAD {
        return Err(AdbError::Protocol(format!(
            "pairing payload of {length} bytes is out of range"
        )));
    }
    let mut payload = vec![0u8; length];
    reader.read_exact(&mut payload)?;
    Ok((kind, payload))
}

/// AES-128-GCM with a key derived from the SPAKE2 secret and one counter per direction.
pub struct PairingCipher {
    key: [u8; 16],
    sent: u64,
    received: u64,
}

impl PairingCipher {
    /// Derives the cipher from the SPAKE2 shared secret.
    ///
    /// # Errors
    ///
    /// Returns [`AdbError::Pairing`] when key derivation fails.
    pub fn new(shared_secret: &[u8]) -> Result<Self, AdbError> {
        let mut key = [0u8; 16];
        Hkdf::<Sha256>::new(None, shared_secret)
            .expand(HKDF_INFO, &mut key)
            .map_err(|_| AdbError::Pairing("cannot derive the pairing key".to_owned()))?;
        Ok(Self {
            key,
            sent: 0,
            received: 0,
        })
    }

    fn nonce(counter: u64) -> [u8; NONCE_LEN] {
        let mut nonce = [0u8; NONCE_LEN];
        nonce[..8].copy_from_slice(&counter.to_le_bytes());
        nonce
    }

    /// Encrypts one record; the result is the ciphertext followed by the 16-byte tag.
    ///
    /// # Errors
    ///
    /// Returns [`AdbError::Pairing`] when encryption fails.
    pub fn seal(&mut self, plaintext: &[u8]) -> Result<Vec<u8>, AdbError> {
        let mut tag = [0u8; TAG_LEN];
        let mut sealed = encrypt_aead(
            Cipher::aes_128_gcm(),
            &self.key,
            Some(&Self::nonce(self.sent)),
            &[],
            plaintext,
            &mut tag,
        )
        .map_err(|error| AdbError::Pairing(error.to_string()))?;
        sealed.extend_from_slice(&tag);
        self.sent += 1;
        Ok(sealed)
    }

    /// Decrypts one record produced by the peer's [`PairingCipher::seal`].
    ///
    /// # Errors
    ///
    /// Returns [`AdbError::Pairing`] when authentication fails, which means the peers used
    /// different pairing codes.
    pub fn open(&mut self, sealed: &[u8]) -> Result<Vec<u8>, AdbError> {
        let split = sealed
            .len()
            .checked_sub(TAG_LEN)
            .ok_or_else(|| AdbError::Pairing("pairing record is too short".to_owned()))?;
        let (ciphertext, tag) = sealed.split_at(split);
        let plaintext = decrypt_aead(
            Cipher::aes_128_gcm(),
            &self.key,
            Some(&Self::nonce(self.received)),
            &[],
            ciphertext,
            tag,
        )
        .map_err(|_| AdbError::Pairing("the pairing code does not match".to_owned()))?;
        self.received += 1;
        Ok(plaintext)
    }
}

/// Packs a public key line into a fixed-size peer-info record.
///
/// # Errors
///
/// Returns [`AdbError::Pairing`] when the line does not fit.
pub fn peer_info_record(kind: u8, data: &[u8]) -> Result<Vec<u8>, AdbError> {
    if data.len() > PEER_INFO_LEN - 1 {
        return Err(AdbError::Pairing("peer info is too large".to_owned()));
    }
    let mut record = vec![0u8; PEER_INFO_LEN];
    record[0] = kind;
    record[1..=data.len()].copy_from_slice(data);
    Ok(record)
}

/// Pairs this app with the device that shows `pairing_code` on `address`.
///
/// `peer_name` labels the registered key in the device's list of trusted computers.
///
/// # Errors
///
/// Returns [`AdbError::Pairing`] for a wrong code, [`AdbError::Tls`] for transport problems and
/// [`AdbError::Io`] for network failures.
pub fn pair(
    address: SocketAddr,
    pairing_code: &str,
    key: &AdbKey,
    peer_name: &str,
) -> Result<(), AdbError> {
    let stream = TcpStream::connect_timeout(&address, TIMEOUT)?;
    stream.set_read_timeout(Some(TIMEOUT))?;
    stream.set_write_timeout(Some(TIMEOUT))?;
    stream.set_nodelay(true)?;
    let mut tls = tls::connect(&tls::client_connector(key)?, stream)?;

    let mut exported = [0u8; EXPORTED_KEY_LEN];
    tls.ssl()
        .export_keying_material(&mut exported, EXPORTED_KEY_LABEL, None)?;
    let mut password = pairing_code.trim().as_bytes().to_vec();
    password.extend_from_slice(&exported);

    let mut spake = Spake2::new(Role::Alice, CLIENT_NAME, SERVER_NAME)?;
    write_packet(
        &mut tls,
        PacketKind::Spake2Message,
        &spake.generate_message(&password)?,
    )?;
    let (kind, their_message) = read_packet(&mut tls).map_err(rejected)?;
    if kind != PacketKind::Spake2Message {
        return Err(AdbError::Protocol("expected a SPAKE2 message".to_owned()));
    }
    let mut cipher = PairingCipher::new(&spake.process_message(&their_message)?)?;

    let record = peer_info_record(PEER_INFO_RSA_PUBLIC_KEY, &key.public_key_line(peer_name)?)?;
    write_packet(&mut tls, PacketKind::PeerInfo, &cipher.seal(&record)?)?;
    let (kind, sealed) = read_packet(&mut tls).map_err(rejected)?;
    if kind != PacketKind::PeerInfo {
        return Err(AdbError::Protocol(
            "expected the device's peer info".to_owned(),
        ));
    }
    let their_info = cipher.open(&sealed)?;
    if their_info.len() != PEER_INFO_LEN {
        return Err(AdbError::Protocol(
            "the device's peer info has the wrong size".to_owned(),
        ));
    }
    Ok(())
}

/// A device that hangs up mid-exchange is how `adbd` reports a wrong pairing code.
fn rejected(error: AdbError) -> AdbError {
    match error {
        AdbError::Io(io_error)
            if matches!(
                io_error.kind(),
                io::ErrorKind::UnexpectedEof
                    | io::ErrorKind::ConnectionReset
                    | io::ErrorKind::BrokenPipe
            ) =>
        {
            AdbError::Pairing("the device closed the connection; check the pairing code".to_owned())
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::io::Cursor;

    #[test]
    fn packets_round_trip() {
        let mut buffer = Vec::new();
        write_packet(&mut buffer, PacketKind::Spake2Message, b"hello").expect("write");
        assert_eq!(&buffer[..6], &[1, 0, 0, 0, 0, 5]);
        let (kind, payload) = read_packet(&mut Cursor::new(buffer)).expect("read");
        assert_eq!(kind, PacketKind::Spake2Message);
        assert_eq!(payload, b"hello");
    }

    #[test]
    fn rejects_bad_headers() {
        for header in [
            [2u8, 0, 0, 0, 0, 1],
            [1, 9, 0, 0, 0, 1],
            [1, 0, 0, 0, 0, 0],
            [1, 0, 0, 1, 0, 1],
        ] {
            let mut bytes = header.to_vec();
            bytes.push(0);
            assert!(
                read_packet(&mut Cursor::new(bytes)).is_err(),
                "{header:?} must be rejected"
            );
        }
    }

    #[test]
    fn ciphers_agree_in_both_directions_with_independent_counters() {
        let mut client = PairingCipher::new(b"shared secret").expect("client");
        let mut server = PairingCipher::new(b"shared secret").expect("server");
        for round in 0..3u8 {
            let sealed = client.seal(&[round; 40]).expect("seal");
            assert_eq!(server.open(&sealed).expect("open"), vec![round; 40]);
        }
        let sealed = server.seal(b"reply").expect("seal");
        assert_eq!(client.open(&sealed).expect("open"), b"reply");
    }

    #[test]
    fn a_wrong_secret_fails_authentication() {
        let mut client = PairingCipher::new(b"one").expect("client");
        let mut server = PairingCipher::new(b"two").expect("server");
        let sealed = client.seal(b"data").expect("seal");
        assert!(matches!(server.open(&sealed), Err(AdbError::Pairing(_))));
    }

    #[test]
    fn replayed_or_reordered_records_fail() {
        let mut client = PairingCipher::new(b"s").expect("client");
        let mut server = PairingCipher::new(b"s").expect("server");
        let first = client.seal(b"first").expect("seal");
        let second = client.seal(b"second").expect("seal");
        assert!(server.open(&second).is_err());
        assert_eq!(server.open(&first).expect("in order"), b"first");
    }

    #[test]
    fn peer_info_records_have_the_fixed_size() {
        let record =
            peer_info_record(PEER_INFO_RSA_PUBLIC_KEY, b"key user@host\0").expect("record");
        assert_eq!(record.len(), PEER_INFO_LEN);
        assert_eq!(record[0], 0);
        assert_eq!(&record[1..15], b"key user@host\0");
        assert!(peer_info_record(0, &vec![1; PEER_INFO_LEN]).is_err());
    }

    proptest! {
        #[test]
        fn any_plaintext_survives_a_seal_open_round_trip(
            secret in proptest::collection::vec(any::<u8>(), 1..80),
            plaintext in proptest::collection::vec(any::<u8>(), 0..300),
        ) {
            let mut client = PairingCipher::new(&secret).expect("client");
            let mut server = PairingCipher::new(&secret).expect("server");
            let sealed = client.seal(&plaintext).expect("seal");
            prop_assert_eq!(server.open(&sealed).expect("open"), plaintext);
        }
    }
}
