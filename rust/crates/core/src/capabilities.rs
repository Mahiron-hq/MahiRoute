// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

//! What a particular firmware build is known to honor.

use serde::{Deserialize, Serialize};

/// Whether a routing mechanism has an effect on this firmware.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Support {
    /// Not measured yet; treated like [`Support::Honored`] until a probe says otherwise.
    #[default]
    Unknown,
    /// A probe stream was routed to the preferred device.
    Honored,
    /// A probe stream ignored the preference, so devices must be masked instead.
    Ignored,
}

impl Support {
    /// Whether the preference alone is expected to route streams.
    #[must_use]
    pub fn preference_suffices(self) -> bool {
        !matches!(self, Self::Ignored)
    }
}

/// Measured behavior of the routing mechanisms on one firmware build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Capabilities {
    /// Effect of the per-capture-preset preferred device.
    pub capture_preset: Support,
    /// Effect of the per-strategy preferred device.
    pub output_strategy: Support,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_is_optimistic() {
        assert!(Support::Unknown.preference_suffices());
        assert!(Support::Honored.preference_suffices());
        assert!(!Support::Ignored.preference_suffices());
    }

    #[test]
    fn default_capabilities_are_unknown() {
        assert_eq!(Capabilities::default().capture_preset, Support::Unknown);
        assert_eq!(Capabilities::default().output_strategy, Support::Unknown);
    }
}
