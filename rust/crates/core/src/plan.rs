// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

//! Turns the user's selection and the connected devices into the routing state to apply.

use serde::{Deserialize, Serialize};

use crate::capabilities::{Capabilities, Support};
use crate::device::{Device, DeviceKind, Direction};
use crate::selection::{Choice, Selection};

/// Enough information for the helper to rebuild `AudioDeviceAttributes` of a device.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DeviceRef {
    /// Recording or playback.
    pub direction: Direction,
    /// Raw `AudioDeviceInfo.TYPE_*` value.
    pub android_type: i32,
    /// Hardware address of the device.
    pub address: String,
}

impl From<&Device> for DeviceRef {
    fn from(device: &Device) -> Self {
        Self {
            direction: device.direction,
            android_type: device.android_type,
            address: device.address.clone(),
        }
    }
}

/// The complete routing state the helper must converge to. Applying it twice is a no-op.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DesiredRouting {
    /// Microphone to prefer for every capture preset.
    pub prefer_input: Option<DeviceRef>,
    /// Speaker to prefer for every product strategy.
    pub prefer_output: Option<DeviceRef>,
    /// Devices to hide from the audio policy so a lower-ranked choice wins.
    pub masked: Vec<DeviceRef>,
}

/// Result of planning: what to apply and which choices cannot be honored right now.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PlanOutcome {
    /// State to apply.
    pub desired: DesiredRouting,
    /// Directions whose selected device is not connected.
    pub missing: Vec<Direction>,
}

fn rank(direction: Direction, device: &Device) -> u8 {
    match direction {
        Direction::Input => device.kind.input_rank(),
        Direction::Output => device.kind.output_rank(),
    }
}

/// Another microphone of the phone itself: in some call routes the platform swaps the chosen one
/// for its sibling (the speakerphone microphone), so the sibling must be hidden too.
fn competes_as_sibling(target: &Device, other: &Device) -> bool {
    target.kind == DeviceKind::BuiltinMic
        && other.kind == DeviceKind::BuiltinMic
        && other.address != target.address
}

fn support(direction: Direction, capabilities: Capabilities) -> Support {
    match direction {
        Direction::Input => capabilities.capture_preset,
        Direction::Output => capabilities.output_strategy,
    }
}

/// Computes the routing state for `selection` given the currently connected `devices`.
///
/// `devices` must include devices this app has masked, otherwise a masked choice would look
/// disconnected. When the firmware ignores a preference (`capabilities`), every device that the
/// platform would rank above the choice is masked as well.
#[must_use]
pub fn plan_route(
    selection: &Selection,
    devices: &[Device],
    capabilities: &Capabilities,
) -> PlanOutcome {
    let mut outcome = PlanOutcome::default();
    if !selection.enabled {
        return outcome;
    }
    for (direction, choice) in [
        (Direction::Input, &selection.input),
        (Direction::Output, &selection.output),
    ] {
        let Choice::Device(key) = choice else {
            continue;
        };
        let Some(target) = devices
            .iter()
            .find(|device| key.matches(device) && device.kind.is_user_selectable())
        else {
            outcome.missing.push(direction);
            continue;
        };
        let target_ref = DeviceRef::from(target);
        match direction {
            Direction::Input => outcome.desired.prefer_input = Some(target_ref),
            Direction::Output => outcome.desired.prefer_output = Some(target_ref),
        }
        if !support(direction, *capabilities).preference_suffices() {
            let target_rank = rank(direction, target);
            outcome.desired.masked.extend(
                devices
                    .iter()
                    .filter(|device| {
                        device.direction == direction
                            && device.port != target.port
                            && device.kind.is_user_selectable()
                            && (rank(direction, device) > target_rank
                                || competes_as_sibling(target, device))
                    })
                    .map(DeviceRef::from),
            );
        }
    }
    outcome.desired.masked.sort();
    outcome.desired.masked.dedup();
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::{DeviceKey, DeviceKind, PortId};
    use proptest::prelude::*;

    fn device(port: u32, direction: Direction, android_type: i32, address: &str) -> Device {
        Device {
            port: PortId(port),
            direction,
            kind: DeviceKind::from_android_type(android_type),
            android_type,
            address: address.to_owned(),
            name: String::new(),
            masked: false,
        }
    }

    fn builtin_mic() -> Device {
        device(16, Direction::Input, 15, "bottom")
    }

    fn headset_mic() -> Device {
        device(1270, Direction::Input, 3, "")
    }

    fn headset_out() -> Device {
        device(1268, Direction::Output, 3, "")
    }

    fn speaker() -> Device {
        device(3, Direction::Output, 2, "")
    }

    fn choose_input(device: &Device) -> Selection {
        Selection {
            input: Choice::Device(device.key()),
            ..Selection::default()
        }
    }

    const IGNORED_CAPTURE: Capabilities = Capabilities {
        capture_preset: Support::Ignored,
        output_strategy: Support::Unknown,
    };

    #[test]
    fn disabled_selection_plans_nothing() {
        let mut selection = choose_input(&builtin_mic());
        selection.enabled = false;
        let outcome = plan_route(
            &selection,
            &[builtin_mic(), headset_mic()],
            &Capabilities::default(),
        );
        assert_eq!(outcome, PlanOutcome::default());
    }

    #[test]
    fn system_default_plans_nothing() {
        let outcome = plan_route(
            &Selection::default(),
            &[builtin_mic(), headset_mic()],
            &Capabilities::default(),
        );
        assert_eq!(outcome, PlanOutcome::default());
    }

    #[test]
    fn preference_alone_is_used_when_the_firmware_honors_it() {
        let devices = [builtin_mic(), headset_mic()];
        let outcome = plan_route(
            &choose_input(&builtin_mic()),
            &devices,
            &Capabilities::default(),
        );
        assert_eq!(
            outcome.desired.prefer_input,
            Some(DeviceRef::from(&builtin_mic()))
        );
        assert!(outcome.desired.masked.is_empty());
        assert!(outcome.missing.is_empty());
    }

    #[test]
    fn higher_ranked_inputs_are_masked_when_the_preference_is_ignored() {
        let devices = [builtin_mic(), headset_mic()];
        let outcome = plan_route(&choose_input(&builtin_mic()), &devices, &IGNORED_CAPTURE);
        assert_eq!(
            outcome.desired.masked,
            vec![DeviceRef::from(&headset_mic())]
        );
        assert_eq!(
            outcome.desired.prefer_input,
            Some(DeviceRef::from(&builtin_mic()))
        );
    }

    #[test]
    fn nothing_is_masked_when_the_choice_already_wins() {
        let devices = [builtin_mic(), headset_mic()];
        let outcome = plan_route(&choose_input(&headset_mic()), &devices, &IGNORED_CAPTURE);
        assert!(outcome.desired.masked.is_empty());
    }

    #[test]
    fn bluetooth_headset_outranks_a_wired_one() {
        let bluetooth = device(40, Direction::Input, 7, "AA:BB:CC:DD:EE:FF");
        let devices = [builtin_mic(), headset_mic(), bluetooth.clone()];
        let outcome = plan_route(&choose_input(&bluetooth), &devices, &IGNORED_CAPTURE);
        assert!(outcome.desired.masked.is_empty());
        let outcome = plan_route(&choose_input(&headset_mic()), &devices, &IGNORED_CAPTURE);
        assert_eq!(outcome.desired.masked, vec![DeviceRef::from(&bluetooth)]);
    }

    #[test]
    fn a_masked_choice_is_still_found_and_unmasked() {
        let mut hidden = headset_mic();
        hidden.masked = true;
        let devices = [builtin_mic(), hidden.clone()];
        let outcome = plan_route(&choose_input(&hidden), &devices, &IGNORED_CAPTURE);
        assert_eq!(outcome.desired.prefer_input, Some(DeviceRef::from(&hidden)));
        assert!(outcome.desired.masked.is_empty());
    }

    #[test]
    fn missing_device_falls_back_and_is_reported() {
        let selection = choose_input(&headset_mic());
        let outcome = plan_route(&selection, &[builtin_mic()], &Capabilities::default());
        assert_eq!(outcome.missing, vec![Direction::Input]);
        assert_eq!(outcome.desired, DesiredRouting::default());
    }

    #[test]
    fn output_choice_masks_higher_ranked_outputs_only_when_ignored() {
        let selection = Selection {
            output: Choice::Device(speaker().key()),
            ..Selection::default()
        };
        let devices = [speaker(), headset_out()];
        let honored = plan_route(&selection, &devices, &Capabilities::default());
        assert!(honored.desired.masked.is_empty());
        let ignored = plan_route(
            &selection,
            &devices,
            &Capabilities {
                output_strategy: Support::Ignored,
                ..Capabilities::default()
            },
        );
        assert_eq!(
            ignored.desired.masked,
            vec![DeviceRef::from(&headset_out())]
        );
    }

    #[test]
    fn the_sibling_builtin_microphone_is_hidden_when_the_preference_is_ignored() {
        let back = device(17, Direction::Input, 15, "back");
        let devices = [builtin_mic(), back.clone(), headset_mic()];
        let outcome = plan_route(&choose_input(&builtin_mic()), &devices, &IGNORED_CAPTURE);
        assert!(outcome.desired.masked.contains(&DeviceRef::from(&back)));
        assert!(
            outcome
                .desired
                .masked
                .contains(&DeviceRef::from(&headset_mic()))
        );
        let outcome = plan_route(&choose_input(&back), &devices, &IGNORED_CAPTURE);
        assert!(
            outcome
                .desired
                .masked
                .contains(&DeviceRef::from(&builtin_mic()))
        );
        assert!(!outcome.desired.masked.contains(&DeviceRef::from(&back)));
        let honored = plan_route(
            &choose_input(&builtin_mic()),
            &devices,
            &Capabilities::default(),
        );
        assert!(honored.desired.masked.is_empty());
    }

    #[test]
    fn virtual_ports_are_never_masked_or_chosen() {
        let telephony = device(20, Direction::Input, 18, "");
        let devices = [builtin_mic(), telephony.clone(), headset_mic()];
        let outcome = plan_route(&choose_input(&builtin_mic()), &devices, &IGNORED_CAPTURE);
        assert_eq!(
            outcome.desired.masked,
            vec![DeviceRef::from(&headset_mic())]
        );
        let selection = Selection {
            input: Choice::Device(DeviceKey {
                direction: Direction::Input,
                kind: DeviceKind::Virtual,
                address: String::new(),
            }),
            ..Selection::default()
        };
        assert_eq!(
            plan_route(&selection, &devices, &IGNORED_CAPTURE).missing,
            vec![Direction::Input]
        );
    }

    fn arbitrary_device() -> impl Strategy<Value = Device> {
        (0u32..50, any::<bool>(), 0i32..32, "[a-z0-9:;=]{0,8}").prop_map(
            |(port, input, android_type, address)| {
                let direction = if input {
                    Direction::Input
                } else {
                    Direction::Output
                };
                device(port, direction, android_type, &address)
            },
        )
    }

    proptest! {
        #[test]
        fn masks_never_include_the_chosen_device(
            devices in proptest::collection::vec(arbitrary_device(), 1..10),
            pick in any::<proptest::sample::Index>(),
        ) {
            let target = pick.get(&devices).clone();
            let selection = Selection {
                input: Choice::Device(target.key()),
                output: Choice::Device(target.key()),
                ..Selection::default()
            };
            let capabilities = Capabilities { capture_preset: Support::Ignored, output_strategy: Support::Ignored };
            let outcome = plan_route(&selection, &devices, &capabilities);
            let chosen: Vec<_> = [&outcome.desired.prefer_input, &outcome.desired.prefer_output]
                .into_iter().flatten().collect();
            for masked in &outcome.desired.masked {
                prop_assert!(!chosen.contains(&masked));
            }
        }

        #[test]
        fn honored_firmware_never_masks(
            devices in proptest::collection::vec(arbitrary_device(), 1..10),
            pick in any::<proptest::sample::Index>(),
        ) {
            let target = pick.get(&devices).clone();
            let selection = Selection {
                input: Choice::Device(target.key()),
                output: Choice::Device(target.key()),
                ..Selection::default()
            };
            let outcome = plan_route(&selection, &devices, &Capabilities::default());
            prop_assert!(outcome.desired.masked.is_empty());
        }

        #[test]
        fn planning_is_deterministic(
            devices in proptest::collection::vec(arbitrary_device(), 0..10),
            pick in any::<proptest::sample::Index>(),
        ) {
            let selection = if devices.is_empty() {
                Selection::default()
            } else {
                Selection { input: Choice::Device(pick.get(&devices).key()), ..Selection::default() }
            };
            let capabilities = Capabilities { capture_preset: Support::Ignored, output_strategy: Support::Ignored };
            prop_assert_eq!(
                plan_route(&selection, &devices, &capabilities),
                plan_route(&selection, &devices, &capabilities)
            );
        }
    }
}
