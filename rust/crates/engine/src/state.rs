// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

//! The read-only view of the engine that the user interface renders.

use mahiroute_core::protocol::StreamInfo;
use mahiroute_core::{Capabilities, Device, Direction, Selection};
use serde::Serialize;

/// Connection to the privileged helper.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum LinkView {
    /// No helper endpoint is known.
    Disconnected,
    /// Connecting or handshaking.
    Connecting,
    /// The helper answers and routing can be applied.
    Ready,
    /// The helper cannot be reached.
    Failed {
        /// Human-readable cause.
        reason: String,
    },
}

/// Whether the audio system really follows the selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum RoutingStatus {
    /// Nothing is overridden: the system decides.
    Inactive,
    /// A routing change is being applied.
    Applying,
    /// Test streams are being opened to check the result.
    Verifying,
    /// Test streams went to the chosen devices.
    Verified,
    /// The selection was applied but the check could not be completed.
    Unverified,
    /// A test stream went somewhere else even after every workaround.
    Failed {
        /// Direction that is not routed as requested.
        direction: Direction,
    },
}

/// Everything the interface needs to draw the main screen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UiState {
    /// Helper connection.
    pub link: LinkView,
    /// Firmware API level, once the helper has introduced itself.
    pub sdk: Option<u32>,
    /// Master switch.
    pub enabled: bool,
    /// What the user chose.
    pub selection: Selection,
    /// Devices the user can pick, including ones this app currently hides.
    pub devices: Vec<Device>,
    /// Directions whose chosen device is not connected.
    pub missing: Vec<Direction>,
    /// The subset of `missing` whose warning the user has closed.
    pub dismissed_missing: Vec<Direction>,
    /// Result of the last verification.
    pub routing: RoutingStatus,
    /// Measured firmware behavior.
    pub capabilities: Capabilities,
    /// Active audio streams of other apps.
    pub streams: Vec<StreamInfo>,
    /// Latest `AudioManager.MODE_*` value.
    pub mode: i32,
    /// Recent non-fatal problems.
    pub warnings: Vec<String>,
}
