// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

//! TLS configuration shared by pairing and the wireless-debugging session.

use std::fmt::Debug;

use boring::ssl::{HandshakeError, SslConnector, SslMethod, SslStream, SslVerifyMode, SslVersion};

use crate::error::AdbError;
use crate::key::AdbKey;

/// Builds a TLS 1.3 client that presents the app's certificate.
///
/// The device's certificate is not verified: `adbd` is authenticated by the pairing code at pair
/// time and, afterwards, only trusts clients, not the other way round.
///
/// # Errors
///
/// Returns [`AdbError::Tls`] when `BoringSSL` rejects the configuration.
pub fn client_connector(key: &AdbKey) -> Result<SslConnector, AdbError> {
    let mut builder = SslConnector::builder(SslMethod::tls())?;
    builder.set_min_proto_version(Some(SslVersion::TLS1_3))?;
    builder.set_max_proto_version(Some(SslVersion::TLS1_3))?;
    builder.set_verify(SslVerifyMode::NONE);
    builder.set_certificate(key.certificate())?;
    builder.set_private_key(key.private_key())?;
    Ok(builder.build())
}

/// Runs the client handshake over `stream`.
///
/// # Errors
///
/// Returns [`AdbError::Tls`] when the handshake fails, for example because `adbd` does not know
/// this client's key.
pub fn connect<S>(connector: &SslConnector, stream: S) -> Result<SslStream<S>, AdbError>
where
    S: std::io::Read + std::io::Write + Debug,
{
    connector
        .configure()?
        .use_server_name_indication(false)
        .verify_hostname(false)
        .connect("adb", stream)
        .map_err(|error: HandshakeError<S>| AdbError::Tls(error.to_string()))
}
