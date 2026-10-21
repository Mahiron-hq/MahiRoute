// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

//! The RSA identity this app presents to `adbd`.
//!
//! One key does three jobs: it signs the self-signed certificate used for the TLS client
//! authentication, and its public half, in Android's own encoding, is what pairing registers
//! with the device.

use std::fs;
use std::io::{self, Write};
use std::path::Path;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use boring::asn1::{Asn1Integer, Asn1Time};
use boring::bn::BigNum;
use boring::hash::MessageDigest;
use boring::pkey::{PKey, Private};
use boring::rsa::Rsa;
use boring::x509::{X509, X509NameBuilder};

use crate::error::AdbError;

const KEY_BITS: u32 = 2048;
const MODULUS_BYTES: usize = 256;
const MODULUS_WORDS: u32 = 64;
const PUBLIC_EXPONENT: u32 = 65_537;
const CERTIFICATE_DAYS: u32 = 3_650;

/// RSA-2048 key plus the self-signed certificate that wraps it for TLS.
pub struct AdbKey {
    private: PKey<Private>,
    certificate: X509,
}

impl AdbKey {
    /// Generates a fresh key and its certificate.
    ///
    /// # Errors
    ///
    /// Returns [`AdbError::Key`] when `BoringSSL` fails to generate or sign.
    pub fn generate() -> Result<Self, AdbError> {
        let rsa = Rsa::generate(KEY_BITS).map_err(key_error)?;
        Self::from_private(PKey::from_rsa(rsa).map_err(key_error)?)
    }

    /// Parses a key from PEM (PKCS#8 or traditional RSA).
    ///
    /// # Errors
    ///
    /// Returns [`AdbError::Key`] when the PEM is malformed or the key is not RSA-2048.
    pub fn from_pem(pem: &[u8]) -> Result<Self, AdbError> {
        let private = PKey::private_key_from_pem(pem).map_err(key_error)?;
        Self::from_private(private)
    }

    /// Serializes the private key as PKCS#8 PEM.
    ///
    /// # Errors
    ///
    /// Returns [`AdbError::Key`] when serialization fails.
    pub fn to_pem(&self) -> Result<Vec<u8>, AdbError> {
        self.private.private_key_to_pem_pkcs8().map_err(key_error)
    }

    /// Loads the key stored at `path`, creating and storing a new one when the file is missing.
    ///
    /// # Errors
    ///
    /// Returns [`AdbError::Key`] when an existing file cannot be parsed and [`AdbError::Io`]
    /// when the file cannot be read or written.
    pub fn load_or_create(path: &Path) -> Result<Self, AdbError> {
        match fs::read(path) {
            Ok(pem) => Self::from_pem(&pem),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let key = Self::generate()?;
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)?;
                }
                write_private_file(path, &key.to_pem()?)?;
                Ok(key)
            }
            Err(error) => Err(error.into()),
        }
    }

    /// The private key, for TLS.
    #[must_use]
    pub fn private_key(&self) -> &PKey<Private> {
        &self.private
    }

    /// The self-signed certificate, for TLS client authentication.
    #[must_use]
    pub fn certificate(&self) -> &X509 {
        &self.certificate
    }

    /// The public key as `adbd` stores it: base64 of Android's binary encoding, a space, `name`
    /// and a terminating NUL.
    ///
    /// # Errors
    ///
    /// Returns [`AdbError::Key`] when the key does not have the shape Android expects.
    pub fn public_key_line(&self, name: &str) -> Result<Vec<u8>, AdbError> {
        let rsa = self.private.rsa().map_err(key_error)?;
        let blob = android_public_key(rsa.n(), rsa.e())?;
        let mut line = STANDARD.encode(blob).into_bytes();
        line.push(b' ');
        line.extend_from_slice(name.as_bytes());
        line.push(0);
        Ok(line)
    }

    fn from_private(private: PKey<Private>) -> Result<Self, AdbError> {
        let rsa = private.rsa().map_err(key_error)?;
        if rsa.n().num_bits() != KEY_BITS.cast_signed() {
            return Err(AdbError::Key(format!("expected an RSA-{KEY_BITS} key")));
        }
        let certificate = self_signed_certificate(&private)?;
        Ok(Self {
            private,
            certificate,
        })
    }
}

fn key_error(error: impl std::fmt::Display) -> AdbError {
    AdbError::Key(error.to_string())
}

fn self_signed_certificate(key: &PKey<Private>) -> Result<X509, AdbError> {
    let mut name = X509NameBuilder::new().map_err(key_error)?;
    name.append_entry_by_text("CN", "00").map_err(key_error)?;
    let name = name.build();
    let mut builder = X509::builder().map_err(key_error)?;
    builder.set_version(2).map_err(key_error)?;
    let serial = BigNum::from_u32(1)
        .and_then(|n| Asn1Integer::from_bn(&n))
        .map_err(key_error)?;
    builder.set_serial_number(&serial).map_err(key_error)?;
    builder.set_subject_name(&name).map_err(key_error)?;
    builder.set_issuer_name(&name).map_err(key_error)?;
    builder.set_pubkey(key).map_err(key_error)?;
    let not_before = Asn1Time::days_from_now(0).map_err(key_error)?;
    let not_after = Asn1Time::days_from_now(CERTIFICATE_DAYS).map_err(key_error)?;
    builder.set_not_before(&not_before).map_err(key_error)?;
    builder.set_not_after(&not_after).map_err(key_error)?;
    builder
        .sign(key, MessageDigest::sha256())
        .map_err(key_error)?;
    Ok(builder.build())
}

/// Android's `RSAPublicKey` struct: word count, `-1/n[0] mod 2^32`, the modulus and `R^2 mod n`
/// as little-endian byte arrays, and the exponent.
fn android_public_key(
    modulus: &boring::bn::BigNumRef,
    exponent: &boring::bn::BigNumRef,
) -> Result<Vec<u8>, AdbError> {
    let expected_exponent = BigNum::from_u32(PUBLIC_EXPONENT).map_err(key_error)?;
    if *exponent != *expected_exponent {
        return Err(AdbError::Key(
            "only the public exponent 65537 is supported".to_owned(),
        ));
    }
    let padded = modulus.to_vec_padded(MODULUS_BYTES).map_err(key_error)?;
    let low_word = u32::from_be_bytes(
        padded[MODULUS_BYTES - 4..]
            .try_into()
            .map_err(|_| AdbError::Key("unexpected modulus size".to_owned()))?,
    );
    if low_word % 2 == 0 {
        return Err(AdbError::Key("the modulus is even".to_owned()));
    }

    let mut context = boring::bn::BigNumContext::new().map_err(key_error)?;
    let two = BigNum::from_u32(2).map_err(key_error)?;
    let exponent_of_r_squared = BigNum::from_u32(2 * KEY_BITS).map_err(key_error)?;
    let mut r_squared = BigNum::new().map_err(key_error)?;
    r_squared
        .mod_exp(&two, &exponent_of_r_squared, modulus, &mut context)
        .map_err(key_error)?;

    let mut blob = Vec::with_capacity(4 + 4 + 2 * MODULUS_BYTES + 4);
    blob.extend_from_slice(&MODULUS_WORDS.to_le_bytes());
    blob.extend_from_slice(&negated_inverse(low_word).to_le_bytes());
    blob.extend_from_slice(&little_endian(modulus)?);
    blob.extend_from_slice(&little_endian(&r_squared)?);
    blob.extend_from_slice(&PUBLIC_EXPONENT.to_le_bytes());
    Ok(blob)
}

/// `-1 / n0 mod 2^32` for odd `n0`, by Newton's iteration on the inverse.
fn negated_inverse(n0: u32) -> u32 {
    let mut inverse: u32 = 1;
    for _ in 0..5 {
        inverse = inverse.wrapping_mul(2u32.wrapping_sub(n0.wrapping_mul(inverse)));
    }
    inverse.wrapping_neg()
}

fn little_endian(value: &boring::bn::BigNumRef) -> Result<Vec<u8>, AdbError> {
    let mut bytes = value.to_vec_padded(MODULUS_BYTES).map_err(key_error)?;
    bytes.reverse();
    Ok(bytes)
}

#[cfg(unix)]
fn write_private_file(path: &Path, contents: &[u8]) -> Result<(), AdbError> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(contents)?;
    Ok(())
}

#[cfg(not(unix))]
fn write_private_file(path: &Path, contents: &[u8]) -> Result<(), AdbError> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(contents)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_key_line_has_the_android_shape() {
        let key = AdbKey::generate().expect("generate");
        let line = key.public_key_line("mahiroute@phone").expect("line");
        assert_eq!(line.last(), Some(&0));
        let text = std::str::from_utf8(&line[..line.len() - 1]).expect("utf-8");
        let (encoded, name) = text.split_once(' ').expect("name separator");
        assert_eq!(name, "mahiroute@phone");
        let blob = STANDARD.decode(encoded).expect("base64");
        assert_eq!(blob.len(), 524);
        assert_eq!(
            u32::from_le_bytes(blob[0..4].try_into().expect("4 bytes")),
            64
        );
        assert_eq!(
            u32::from_le_bytes(blob[520..524].try_into().expect("4 bytes")),
            65_537
        );
    }

    #[test]
    fn blob_matches_the_modulus_and_its_montgomery_constants() {
        let key = AdbKey::generate().expect("generate");
        let rsa = key.private_key().rsa().expect("rsa");
        let blob = android_public_key(rsa.n(), rsa.e()).expect("blob");

        let mut modulus: Vec<u8> = blob[8..8 + 256].to_vec();
        modulus.reverse();
        assert_eq!(BigNum::from_slice(&modulus).expect("modulus"), *rsa.n());

        // n0inv * n[0] == -1 (mod 2^32).
        let n0inv = u32::from_le_bytes(blob[4..8].try_into().expect("4 bytes"));
        let n_low = u32::from_le_bytes(blob[8..12].try_into().expect("4 bytes"));
        assert_eq!(n0inv.wrapping_mul(n_low), u32::MAX);

        // rr * 1 (mod n) is R^2 mod n: check it against a direct computation.
        let mut rr: Vec<u8> = blob[8 + 256..8 + 512].to_vec();
        rr.reverse();
        let rr = BigNum::from_slice(&rr).expect("rr");
        let mut context = boring::bn::BigNumContext::new().expect("context");
        let two = BigNum::from_u32(2).expect("two");
        let exponent = BigNum::from_u32(4096).expect("exponent");
        let mut expected = BigNum::new().expect("expected");
        expected
            .mod_exp(&two, &exponent, rsa.n(), &mut context)
            .expect("mod_exp");
        assert_eq!(rr, expected);
    }

    #[test]
    fn pem_round_trip_keeps_the_identity() {
        let key = AdbKey::generate().expect("generate");
        let again = AdbKey::from_pem(&key.to_pem().expect("pem")).expect("parse");
        assert_eq!(
            key.public_key_line("a").expect("line"),
            again.public_key_line("a").expect("line")
        );
    }

    #[test]
    fn load_or_create_persists_one_key() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("keys").join("adbkey.pem");
        let first = AdbKey::load_or_create(&path).expect("create");
        let second = AdbKey::load_or_create(&path).expect("load");
        assert_eq!(
            first.public_key_line("x").expect("line"),
            second.public_key_line("x").expect("line")
        );
    }

    #[test]
    fn rejects_keys_of_the_wrong_size() {
        let small = PKey::from_rsa(Rsa::generate(1024).expect("rsa")).expect("pkey");
        let pem = small.private_key_to_pem_pkcs8().expect("pem");
        assert!(matches!(AdbKey::from_pem(&pem), Err(AdbError::Key(_))));
    }

    #[test]
    fn certificate_is_signed_by_the_same_key() {
        let key = AdbKey::generate().expect("generate");
        let public = key.certificate().public_key().expect("public key");
        assert!(key.certificate().verify(&public).expect("verify"));
        assert!(public.public_eq(key.private_key()));
    }
}
