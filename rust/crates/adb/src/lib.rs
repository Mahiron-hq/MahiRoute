// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

//! A small ADB client for the phone it runs on.
//!
//! It implements what the app needs to start its privileged helper without a computer: the
//! wireless-debugging pairing protocol ([`pairing`]), the TLS transport of wireless debugging
//! ([`session`]) and the key material both rely on ([`key`]).

pub mod error;
pub mod helper;
pub mod key;
pub mod pairing;
pub mod session;
pub mod spake2;
pub mod tls;
pub mod wire;

pub use error::AdbError;
pub use helper::{HelperEndpoint, start_helper};
pub use key::AdbKey;
pub use pairing::pair;
pub use session::{Session, run_shell};
