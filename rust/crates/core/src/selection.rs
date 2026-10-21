// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

//! What the user asked for.

use serde::{Deserialize, Serialize};

use crate::device::DeviceKey;

/// The user's decision for one direction.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "device")]
pub enum Choice {
    /// Let Android decide.
    #[default]
    SystemDefault,
    /// Use this device whenever it is connected.
    Device(DeviceKey),
}

/// Input and output choices plus the master switch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Selection {
    /// When false every preference is removed and Android decides.
    pub enabled: bool,
    /// Microphone choice.
    pub input: Choice,
    /// Speaker choice.
    pub output: Choice,
}

impl Default for Selection {
    fn default() -> Self {
        Self {
            enabled: true,
            input: Choice::SystemDefault,
            output: Choice::SystemDefault,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::{DeviceKind, Direction};

    #[test]
    fn round_trips_through_json() {
        let selection = Selection {
            enabled: true,
            input: Choice::Device(DeviceKey {
                direction: Direction::Input,
                kind: DeviceKind::BuiltinMic,
                address: "bottom".to_owned(),
            }),
            output: Choice::SystemDefault,
        };
        let json = serde_json::to_string(&selection).expect("serialize");
        assert_eq!(
            serde_json::from_str::<Selection>(&json).expect("deserialize"),
            selection
        );
    }

    #[test]
    fn default_selection_defers_to_the_system() {
        let selection = Selection::default();
        assert!(selection.enabled);
        assert_eq!(selection.input, Choice::SystemDefault);
        assert_eq!(selection.output, Choice::SystemDefault);
    }
}
