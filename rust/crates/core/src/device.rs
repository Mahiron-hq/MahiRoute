// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

//! Audio devices as reported by the privileged helper.

use serde::{Deserialize, Serialize};

/// Identifier the audio framework gives a device port (`AudioDeviceInfo.getId()`).
///
/// It changes whenever the device is reconnected, so it is never persisted; use
/// [`DeviceKey`] for anything that must survive a reconnect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PortId(pub u32);

/// Whether a device records or plays sound.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// A microphone-like source.
    Input,
    /// A speaker-like sink.
    Output,
}

/// Coarse device category derived from `AudioDeviceInfo.getType()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceKind {
    /// Phone earpiece.
    BuiltinEarpiece,
    /// Main loudspeaker.
    BuiltinSpeaker,
    /// Protected loudspeaker path.
    BuiltinSpeakerSafe,
    /// Microphone built into the phone.
    BuiltinMic,
    /// Wired headset with a microphone.
    WiredHeadset,
    /// Wired headphones.
    WiredHeadphones,
    /// Analog line jack.
    LineAnalog,
    /// Digital line connector.
    LineDigital,
    /// USB headset.
    UsbHeadset,
    /// Generic USB audio device (DAC, microphone).
    UsbDevice,
    /// USB accessory.
    UsbAccessory,
    /// Bluetooth hands-free profile.
    BluetoothSco,
    /// Bluetooth stereo profile.
    BluetoothA2dp,
    /// Bluetooth LE Audio headset.
    BleHeadset,
    /// Bluetooth LE Audio speaker.
    BleSpeaker,
    /// Bluetooth LE Audio broadcast sink.
    BleBroadcast,
    /// Hearing aid.
    HearingAid,
    /// HDMI family of outputs.
    Hdmi,
    /// Docking station.
    Dock,
    /// Auxiliary line.
    AuxLine,
    /// Virtual or special-purpose port that users cannot pick.
    Virtual,
}

impl DeviceKind {
    /// Maps an `AudioDeviceInfo.TYPE_*` constant to a kind.
    #[must_use]
    pub fn from_android_type(android_type: i32) -> Self {
        match android_type {
            1 => Self::BuiltinEarpiece,
            2 => Self::BuiltinSpeaker,
            3 => Self::WiredHeadset,
            4 => Self::WiredHeadphones,
            5 => Self::LineAnalog,
            6 => Self::LineDigital,
            7 => Self::BluetoothSco,
            8 => Self::BluetoothA2dp,
            9 | 10 | 29 => Self::Hdmi,
            11 => Self::UsbDevice,
            12 => Self::UsbAccessory,
            13 | 31 => Self::Dock,
            15 => Self::BuiltinMic,
            19 => Self::AuxLine,
            22 => Self::UsbHeadset,
            23 => Self::HearingAid,
            24 => Self::BuiltinSpeakerSafe,
            26 => Self::BleHeadset,
            27 => Self::BleSpeaker,
            30 => Self::BleBroadcast,
            // Telephony, FM, tuners, IP, bus, remote submix, echo reference and unknown types.
            _ => Self::Virtual,
        }
    }

    /// Whether users can pick this kind of device in the interface.
    #[must_use]
    pub fn is_user_selectable(self) -> bool {
        !matches!(
            self,
            Self::Virtual | Self::BuiltinSpeakerSafe | Self::BleBroadcast | Self::UsbAccessory
        )
    }

    /// Whether the address of this kind stays the same after a reconnect.
    ///
    /// USB addresses encode a card number that the kernel may renumber.
    #[must_use]
    pub fn has_stable_address(self) -> bool {
        !matches!(
            self,
            Self::UsbHeadset | Self::UsbDevice | Self::UsbAccessory
        )
    }

    /// Position in the platform's default selection order for recording: a higher rank wins.
    #[must_use]
    pub fn input_rank(self) -> u8 {
        match self {
            Self::BluetoothSco | Self::BleHeadset | Self::HearingAid => 5,
            Self::WiredHeadset => 4,
            Self::UsbHeadset => 3,
            Self::UsbDevice | Self::LineAnalog | Self::LineDigital | Self::AuxLine | Self::Dock => {
                2
            }
            _ => 0,
        }
    }

    /// Position in the platform's default selection order for playback: a higher rank wins.
    #[must_use]
    pub fn output_rank(self) -> u8 {
        match self {
            Self::HearingAid => 7,
            Self::BleHeadset | Self::BleSpeaker | Self::BluetoothA2dp => 6,
            Self::BluetoothSco => 5,
            Self::WiredHeadset | Self::WiredHeadphones => 4,
            Self::UsbHeadset | Self::UsbDevice | Self::LineAnalog | Self::LineDigital => 3,
            Self::Hdmi | Self::Dock | Self::AuxLine => 2,
            Self::BuiltinSpeaker | Self::BuiltinSpeakerSafe => 1,
            _ => 0,
        }
    }
}

/// A connected audio device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Device {
    /// Current port identifier.
    pub port: PortId,
    /// Recording or playback.
    pub direction: Direction,
    /// Category of the device.
    pub kind: DeviceKind,
    /// Raw `AudioDeviceInfo.TYPE_*` value.
    pub android_type: i32,
    /// Hardware address (MAC address, USB card, microphone position, or empty).
    pub address: String,
    /// Product name reported by the framework.
    pub name: String,
    /// The device is physically connected but hidden from the audio policy by this app.
    pub masked: bool,
}

impl Device {
    /// What tells this device apart from another of the same kind after a reconnect: its
    /// address, or its product name when the kind's address is renumbered (USB).
    #[must_use]
    pub fn identity(&self) -> &str {
        if self.kind.has_stable_address() {
            &self.address
        } else {
            &self.name
        }
    }

    /// Stable identity of the device for persistence and matching.
    #[must_use]
    pub fn key(&self) -> DeviceKey {
        DeviceKey {
            direction: self.direction,
            kind: self.kind,
            address: self.identity().to_owned(),
        }
    }
}

/// Reconnect-proof identity of a device.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DeviceKey {
    /// Recording or playback.
    pub direction: Direction,
    /// Category of the device.
    pub kind: DeviceKind,
    /// Hardware address, or the product name for kinds whose address is renumbered.
    pub address: String,
}

impl DeviceKey {
    /// Whether `device` is the device this key identifies.
    #[must_use]
    pub fn matches(&self, device: &Device) -> bool {
        self.direction == device.direction
            && self.kind == device.kind
            && self.address == device.identity()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn device(direction: Direction, android_type: i32, address: &str) -> Device {
        named(direction, android_type, address, "")
    }

    fn named(direction: Direction, android_type: i32, address: &str, name: &str) -> Device {
        Device {
            port: PortId(7),
            direction,
            kind: DeviceKind::from_android_type(android_type),
            android_type,
            address: address.to_owned(),
            name: name.to_owned(),
            masked: false,
        }
    }

    #[test]
    fn maps_common_android_types() {
        assert_eq!(DeviceKind::from_android_type(15), DeviceKind::BuiltinMic);
        assert_eq!(DeviceKind::from_android_type(3), DeviceKind::WiredHeadset);
        assert_eq!(DeviceKind::from_android_type(22), DeviceKind::UsbHeadset);
        assert_eq!(DeviceKind::from_android_type(26), DeviceKind::BleHeadset);
        assert_eq!(DeviceKind::from_android_type(10), DeviceKind::Hdmi);
    }

    #[test]
    fn telephony_and_unknown_types_are_virtual() {
        for android_type in [0, 14, 16, 18, 25, 28, 999, -1] {
            assert_eq!(
                DeviceKind::from_android_type(android_type),
                DeviceKind::Virtual
            );
        }
    }

    #[test]
    fn virtual_ports_are_not_selectable() {
        assert!(!DeviceKind::Virtual.is_user_selectable());
        assert!(DeviceKind::BuiltinMic.is_user_selectable());
        assert!(DeviceKind::BluetoothSco.is_user_selectable());
    }

    #[test]
    fn wired_headset_outranks_builtin_mic() {
        assert!(DeviceKind::WiredHeadset.input_rank() > DeviceKind::BuiltinMic.input_rank());
        assert!(
            DeviceKind::WiredHeadphones.output_rank() > DeviceKind::BuiltinSpeaker.output_rank()
        );
    }

    #[test]
    fn key_ignores_port_and_volatile_usb_address() {
        let before = named(Direction::Input, 22, "card=1;device=0", "USB Audio");
        let mut after = named(Direction::Input, 22, "card=3;device=0", "USB Audio");
        after.port = PortId(900);
        assert!(before.key().matches(&after));
    }

    #[test]
    fn key_distinguishes_usb_products() {
        let dac = named(Direction::Output, 11, "card=1;device=0", "USB-C DAC");
        let headset = named(Direction::Output, 11, "card=1;device=0", "Gaming Headset");
        assert!(dac.key().matches(&dac));
        assert!(!dac.key().matches(&headset));
    }

    #[test]
    fn key_distinguishes_bluetooth_addresses() {
        let first = device(Direction::Output, 8, "AA:BB:CC:DD:EE:01");
        let second = device(Direction::Output, 8, "AA:BB:CC:DD:EE:02");
        assert!(first.key().matches(&first));
        assert!(!first.key().matches(&second));
    }

    #[test]
    fn key_distinguishes_direction() {
        let input = device(Direction::Input, 3, "");
        let output = device(Direction::Output, 3, "");
        assert!(!input.key().matches(&output));
    }

    proptest! {
        #[test]
        fn every_android_type_maps_without_panicking(android_type in any::<i32>()) {
            let kind = DeviceKind::from_android_type(android_type);
            let _ = (kind.input_rank(), kind.output_rank(), kind.is_user_selectable());
        }

        #[test]
        fn key_always_matches_its_own_device(android_type in 0i32..40, address in "[ -~]{0,24}") {
            let device = device(Direction::Input, android_type, &address);
            prop_assert!(device.key().matches(&device));
        }
    }
}
