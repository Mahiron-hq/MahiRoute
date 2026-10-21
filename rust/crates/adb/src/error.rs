// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

//! Error type shared by the ADB client modules.

use std::io;

use thiserror::Error;

/// Why an ADB operation failed.
#[derive(Debug, Error)]
pub enum AdbError {
    /// A socket operation failed or timed out.
    #[error("network error: {0}")]
    Io(#[from] io::Error),
    /// Setting up or running the TLS connection failed.
    #[error("TLS error: {0}")]
    Tls(String),
    /// The key or certificate could not be created, parsed or stored.
    #[error("key error: {0}")]
    Key(String),
    /// The pairing handshake did not complete, usually because of a wrong pairing code.
    #[error("pairing failed: {0}")]
    Pairing(String),
    /// The peer sent something that is not valid ADB.
    #[error("protocol error: {0}")]
    Protocol(String),
    /// The command ran but could not be started or finished.
    #[error("command failed: {0}")]
    Command(String),
}

impl From<boring::error::ErrorStack> for AdbError {
    fn from(error: boring::error::ErrorStack) -> Self {
        Self::Tls(error.to_string())
    }
}
