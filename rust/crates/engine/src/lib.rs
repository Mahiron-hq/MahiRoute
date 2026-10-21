// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

//! The engine keeps the audio system's routing in line with the user's selection.
//!
//! [`Core`] is a pure state machine; [`Engine`] drives it with a thread, timers and a
//! [`Connector`] to the privileged helper.

pub mod core;
pub mod driver;
pub mod io;
pub mod state;

pub use core::{Command, Core, Effect, HelperTarget, Timer};
pub use driver::{Engine, Listener};
pub use io::{Connector, LineSource, TcpConnector, Transport};
pub use state::{LinkView, RoutingStatus, UiState};
