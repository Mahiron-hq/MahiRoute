// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

//! End-to-end behavior of the engine core against a model of the privileged helper.

use std::collections::VecDeque;
use std::time::Duration;

use mahiroute_core::protocol::{
    Event, RawDevice, Request, RequestBody, Response, ResponseBody, encode_line,
};
use mahiroute_core::{
    Choice, DesiredRouting, DeviceKey, DeviceKind, DeviceRef, Direction, Settings, Support,
};
use mahiroute_engine::{Command, Core, Effect, HelperTarget, LinkView, RoutingStatus, Timer};

const BUILTIN_MIC: i32 = 15;
const WIRED_HEADSET: i32 = 3;
const SPEAKER: i32 = 2;

fn raw(port: u32, input: bool, android_type: i32, address: &str) -> RawDevice {
    RawDevice {
        port,
        input,
        android_type,
        address: address.to_owned(),
        name: "model".to_owned(),
        masked: false,
    }
}

fn phone_with_headphones() -> Vec<RawDevice> {
    vec![
        raw(2, false, 1, ""),
        raw(3, false, SPEAKER, ""),
        raw(1268, false, WIRED_HEADSET, ""),
        raw(16, true, BUILTIN_MIC, "bottom"),
        raw(1270, true, WIRED_HEADSET, ""),
        raw(20, true, 18, ""),
    ]
}

fn key(direction: Direction, android_type: i32, address: &str) -> DeviceKey {
    DeviceKey {
        direction,
        kind: DeviceKind::from_android_type(android_type),
        address: address.to_owned(),
    }
}

fn rank(android_type: i32, input: bool) -> u8 {
    let kind = DeviceKind::from_android_type(android_type);
    if input {
        kind.input_rank()
    } else {
        kind.output_rank()
    }
}

/// Model of the helper: hides devices on request and routes probe streams like a firmware would.
struct FakeHelper {
    visible: Vec<RawDevice>,
    hidden: Vec<RawDevice>,
    honors_preferences: bool,
    routes_correctly_with_masks: bool,
    preferred_input: Option<DeviceRef>,
    preferred_output: Option<DeviceRef>,
    fingerprint: String,
    applies: Vec<DesiredRouting>,
    probes: usize,
}

impl FakeHelper {
    fn new(honors_preferences: bool) -> Self {
        Self {
            visible: phone_with_headphones(),
            hidden: Vec::new(),
            honors_preferences,
            routes_correctly_with_masks: true,
            preferred_input: None,
            preferred_output: None,
            fingerprint: "vendor/model:14".to_owned(),
            applies: Vec::new(),
            probes: 0,
        }
    }

    fn reply(id: u64, body: ResponseBody) -> String {
        encode_line(&Response { id, body })
            .expect("encode")
            .trim_end()
            .to_owned()
    }

    fn route(&self, input: bool, preferred: Option<&DeviceRef>) -> Option<&RawDevice> {
        let candidates = self.visible.iter().filter(|device| {
            device.input == input
                && DeviceKind::from_android_type(device.android_type).is_user_selectable()
        });
        if self.honors_preferences
            && let Some(wanted) = preferred
            && let Some(device) = self.visible.iter().find(|device| {
                device.android_type == wanted.android_type && device.address == wanted.address
            })
        {
            return Some(device);
        }
        candidates.max_by_key(|device| rank(device.android_type, input))
    }

    fn handle(&mut self, request: &Request) -> Option<String> {
        let id = request.id;
        Some(match &request.body {
            RequestBody::Hello { .. } => Self::reply(
                id,
                ResponseBody::Hello {
                    version: 1,
                    sdk: 33,
                    fingerprint: self.fingerprint.clone(),
                    uid: 2000,
                },
            ),
            RequestBody::ListDevices => Self::reply(
                id,
                ResponseBody::Devices {
                    devices: self.all_devices(),
                },
            ),
            RequestBody::Apply { desired } => {
                self.applies.push(desired.clone());
                self.apply(desired);
                Self::reply(
                    id,
                    ResponseBody::Applied {
                        warnings: Vec::new(),
                    },
                )
            }
            RequestBody::ClearAll => Self::reply(id, ResponseBody::Cleared),
            RequestBody::Diagnostics => Self::reply(
                id,
                ResponseBody::Diagnostics {
                    streams: Vec::new(),
                },
            ),
            RequestBody::Probe { direction, .. } => {
                self.probes += 1;
                let input = *direction == Direction::Input;
                let preferred = if input {
                    &self.preferred_input
                } else {
                    &self.preferred_output
                };
                let routed = if self.routes_correctly_with_masks {
                    self.route(input, preferred.as_ref())
                } else {
                    // A firmware that keeps using the headset no matter what is hidden.
                    self.visible
                        .iter()
                        .chain(self.hidden.iter())
                        .find(|device| {
                            device.input == input && device.android_type == WIRED_HEADSET
                        })
                };
                Self::reply(
                    id,
                    ResponseBody::Probed {
                        routed_port: routed.map(|device| device.port),
                        routed_type: routed.map(|device| device.android_type),
                        routed_address: routed.map(|device| device.address.clone()),
                        level: None,
                    },
                )
            }
            RequestBody::Shutdown => return None,
        })
    }

    fn all_devices(&self) -> Vec<RawDevice> {
        let mut devices = self.visible.clone();
        devices.extend(self.hidden.iter().cloned().map(|mut device| {
            device.masked = true;
            device
        }));
        devices
    }

    fn apply(&mut self, desired: &DesiredRouting) {
        self.preferred_input.clone_from(&desired.prefer_input);
        self.preferred_output.clone_from(&desired.prefer_output);
        let mut all = std::mem::take(&mut self.visible);
        all.append(&mut self.hidden);
        for device in all {
            let hide = desired.masked.iter().any(|masked| {
                masked.android_type == device.android_type
                    && masked.address == device.address
                    && (masked.direction == Direction::Input) == device.input
            });
            if hide {
                self.hidden.push(device);
            } else {
                self.visible.push(device);
            }
        }
    }
}

struct Harness {
    core: Core,
    helper: FakeHelper,
    sent: Vec<RequestBody>,
    scheduled: Vec<(Timer, Duration)>,
}

impl Harness {
    fn new(settings: Settings, helper: FakeHelper) -> Self {
        Self {
            core: Core::new(settings),
            helper,
            sent: Vec::new(),
            scheduled: Vec::new(),
        }
    }

    fn run(&mut self, effects: Vec<Effect>) {
        let mut queue = VecDeque::from(effects);
        while let Some(effect) = queue.pop_front() {
            match effect {
                Effect::Send(request) => {
                    self.sent.push(request.body.clone());
                    if let Some(line) = self.helper.handle(&request) {
                        queue.extend(self.core.incoming(&line));
                    }
                }
                Effect::Connect(_) => queue.extend(self.core.connected()),
                Effect::Schedule(timer, delay) => self.scheduled.push((timer, delay)),
                Effect::Close | Effect::Cancel(_) | Effect::Persist | Effect::Emit => {}
            }
        }
    }

    fn command(&mut self, command: Command) {
        let effects = self.core.command(command);
        self.run(effects);
    }

    fn connect(&mut self) {
        self.command(Command::Connect(HelperTarget {
            port: 40000,
            token: "secret-secret-secret".to_owned(),
        }));
    }

    fn push_devices(&mut self) {
        let line = encode_line(&Event::Devices {
            devices: self.helper.all_devices(),
        })
        .expect("encode");
        let effects = self.core.incoming(line.trim_end());
        self.run(effects);
    }

    fn choose_builtin_mic(&mut self) {
        self.command(Command::Choose {
            direction: Direction::Input,
            choice: Choice::Device(key(Direction::Input, BUILTIN_MIC, "bottom")),
        });
    }
}

#[test]
fn firmware_that_honors_preferences_needs_no_masking() {
    let mut harness = Harness::new(Settings::default(), FakeHelper::new(true));
    harness.connect();
    harness.choose_builtin_mic();

    let state = harness.core.ui_state();
    assert_eq!(state.link, LinkView::Ready);
    assert_eq!(state.routing, RoutingStatus::Verified);
    assert_eq!(state.capabilities.capture_preset, Support::Honored);
    let last = harness.helper.applies.last().expect("applied");
    assert!(last.prefer_input.is_some());
    assert!(last.masked.is_empty());
}

#[test]
fn firmware_that_ignores_preferences_gets_the_blocking_device_masked() {
    let mut harness = Harness::new(Settings::default(), FakeHelper::new(false));
    harness.connect();
    harness.choose_builtin_mic();

    let state = harness.core.ui_state();
    assert_eq!(state.routing, RoutingStatus::Verified);
    assert_eq!(state.capabilities.capture_preset, Support::Ignored);
    let last = harness.helper.applies.last().expect("applied");
    assert_eq!(last.masked.len(), 1);
    assert_eq!(last.masked[0].android_type, WIRED_HEADSET);
    assert_eq!(last.masked[0].direction, Direction::Input);
    assert_eq!(
        harness
            .core
            .settings()
            .capabilities_for("vendor/model:14")
            .capture_preset,
        Support::Ignored
    );
}

#[test]
fn learned_capabilities_skip_the_failed_first_attempt() {
    let mut first = Harness::new(Settings::default(), FakeHelper::new(false));
    first.connect();
    first.choose_builtin_mic();
    let remembered = first.core.settings().clone();

    let mut second = Harness::new(remembered, FakeHelper::new(false));
    second.connect();
    assert_eq!(
        second.helper.applies.len(),
        1,
        "one apply, with the masks already in it"
    );
    assert!(!second.helper.applies[0].masked.is_empty());
    assert_eq!(second.core.ui_state().routing, RoutingStatus::Verified);
}

#[test]
fn hidden_devices_are_unmasked_when_the_selection_is_cleared() {
    let mut harness = Harness::new(Settings::default(), FakeHelper::new(false));
    harness.connect();
    harness.choose_builtin_mic();
    assert!(!harness.helper.hidden.is_empty());

    harness.command(Command::Choose {
        direction: Direction::Input,
        choice: Choice::SystemDefault,
    });
    assert!(harness.helper.hidden.is_empty());
    assert_eq!(harness.core.ui_state().routing, RoutingStatus::Inactive);
}

#[test]
fn disabling_removes_every_override() {
    let mut harness = Harness::new(Settings::default(), FakeHelper::new(false));
    harness.connect();
    harness.choose_builtin_mic();
    harness.command(Command::SetEnabled(false));

    assert!(harness.helper.hidden.is_empty());
    assert_eq!(harness.helper.preferred_input, None);
    assert_eq!(harness.core.ui_state().routing, RoutingStatus::Inactive);
    assert!(!harness.core.ui_state().enabled);
}

#[test]
fn a_disconnected_device_is_reported_and_restored_when_it_returns() {
    let mut harness = Harness::new(Settings::default(), FakeHelper::new(true));
    harness.connect();
    harness.command(Command::Choose {
        direction: Direction::Input,
        choice: Choice::Device(key(Direction::Input, WIRED_HEADSET, "")),
    });
    assert!(harness.core.ui_state().missing.is_empty());

    harness
        .helper
        .visible
        .retain(|device| !(device.input && device.android_type == WIRED_HEADSET));
    harness.push_devices();
    let state = harness.core.ui_state();
    assert_eq!(state.missing, vec![Direction::Input]);
    assert_eq!(
        harness.helper.preferred_input, None,
        "falls back to the system default"
    );

    harness
        .helper
        .visible
        .push(raw(1400, true, WIRED_HEADSET, ""));
    harness.push_devices();
    assert!(harness.core.ui_state().missing.is_empty());
    assert!(harness.helper.preferred_input.is_some());
}

#[test]
fn a_routing_that_never_takes_effect_is_reported_without_looping() {
    let mut helper = FakeHelper::new(false);
    helper.routes_correctly_with_masks = false;
    let mut harness = Harness::new(Settings::default(), helper);
    harness.connect();
    harness.choose_builtin_mic();

    assert_eq!(
        harness.core.ui_state().routing,
        RoutingStatus::Failed {
            direction: Direction::Input
        }
    );
    assert!(
        harness.helper.probes <= 8,
        "probing must stop after the escalation, got {}",
        harness.helper.probes
    );
    assert!(!harness.core.ui_state().warnings.is_empty());
}

#[test]
fn output_selection_is_verified_and_escalated_like_input() {
    let mut harness = Harness::new(Settings::default(), FakeHelper::new(false));
    harness.connect();
    harness.command(Command::Choose {
        direction: Direction::Output,
        choice: Choice::Device(key(Direction::Output, SPEAKER, "")),
    });
    let state = harness.core.ui_state();
    assert_eq!(state.routing, RoutingStatus::Verified);
    assert_eq!(state.capabilities.output_strategy, Support::Ignored);
    let last = harness.helper.applies.last().expect("applied");
    assert_eq!(last.masked.len(), 1);
    assert_eq!(last.masked[0].direction, Direction::Output);
}

#[test]
fn a_dead_helper_is_detected_and_reconnected_with_backoff() {
    let mut harness = Harness::new(Settings::default(), FakeHelper::new(true));
    harness.connect();
    let effects = harness.core.timer(Timer::Heartbeat);
    harness.run(effects);
    // The fake answers instantly, so simulate silence by firing the deadline while one is open.
    let effects = harness.core.timer(Timer::Heartbeat);
    assert!(
        effects
            .iter()
            .any(|effect| matches!(effect, Effect::Send(_)))
    );
    let effects = harness.core.timer(Timer::HeartbeatDeadline);
    assert!(
        effects
            .iter()
            .any(|effect| matches!(effect, Effect::Schedule(Timer::Reconnect, _)))
    );
    assert!(matches!(
        harness.core.ui_state().link,
        LinkView::Failed { .. }
    ));

    let delays: Vec<Duration> = (0..3)
        .map(|_| {
            let effects = harness.core.closed("again");
            effects
                .into_iter()
                .find_map(|effect| match effect {
                    Effect::Schedule(Timer::Reconnect, delay) => Some(delay),
                    _ => None,
                })
                .expect("reconnect scheduled")
        })
        .collect();
    assert!(
        delays.windows(2).all(|pair| pair[0] <= pair[1]),
        "backoff must not shrink: {delays:?}"
    );
}

fn headset_mic_key() -> Choice {
    Choice::Device(key(Direction::Input, WIRED_HEADSET, ""))
}

fn unplug_headset_mic(helper: &mut FakeHelper) {
    helper
        .visible
        .retain(|device| !(device.input && device.android_type == WIRED_HEADSET));
}

#[test]
fn a_closed_warning_stays_closed_across_restarts_until_the_device_returns() {
    let mut harness = Harness::new(Settings::default(), FakeHelper::new(true));
    harness.connect();
    harness.command(Command::Choose {
        direction: Direction::Input,
        choice: headset_mic_key(),
    });
    unplug_headset_mic(&mut harness.helper);
    harness.push_devices();
    assert_eq!(harness.core.ui_state().missing, vec![Direction::Input]);
    assert!(harness.core.ui_state().dismissed_missing.is_empty());

    let effects = harness
        .core
        .command(Command::DismissMissing(Direction::Input));
    assert!(effects.contains(&Effect::Persist));
    assert_eq!(
        harness.core.ui_state().dismissed_missing,
        vec![Direction::Input]
    );

    // A fresh process: the engine is rebuilt from the stored settings while the device is still gone.
    let mut helper = FakeHelper::new(true);
    unplug_headset_mic(&mut helper);
    let mut restarted = Harness::new(harness.core.settings().clone(), helper);
    restarted.connect();
    let state = restarted.core.ui_state();
    assert_eq!(state.missing, vec![Direction::Input]);
    assert_eq!(state.dismissed_missing, vec![Direction::Input]);

    // The device comes back: routing resumes and a later unplug warns again.
    restarted
        .helper
        .visible
        .push(raw(1400, true, WIRED_HEADSET, ""));
    restarted.push_devices();
    assert!(restarted.core.ui_state().missing.is_empty());
    assert!(restarted.helper.preferred_input.is_some());
    assert!(restarted.core.settings().dismissed_missing.is_empty());
    unplug_headset_mic(&mut restarted.helper);
    restarted.push_devices();
    let state = restarted.core.ui_state();
    assert_eq!(state.missing, vec![Direction::Input]);
    assert!(state.dismissed_missing.is_empty());
}

#[test]
fn choosing_another_device_forgets_the_closed_warning() {
    let mut harness = Harness::new(Settings::default(), FakeHelper::new(true));
    harness.connect();
    harness.command(Command::Choose {
        direction: Direction::Input,
        choice: headset_mic_key(),
    });
    unplug_headset_mic(&mut harness.helper);
    harness.push_devices();
    harness.command(Command::DismissMissing(Direction::Input));

    harness.choose_builtin_mic();
    let state = harness.core.ui_state();
    assert!(state.missing.is_empty());
    assert!(harness.core.settings().dismissed_missing.is_empty());

    // Going back to the vanished device is a new decision and shows the warning again.
    harness.command(Command::Choose {
        direction: Direction::Input,
        choice: headset_mic_key(),
    });
    let state = harness.core.ui_state();
    assert_eq!(state.missing, vec![Direction::Input]);
    assert!(state.dismissed_missing.is_empty());
}

#[test]
fn closing_a_warning_that_is_not_shown_changes_nothing() {
    let mut harness = Harness::new(Settings::default(), FakeHelper::new(true));
    harness.connect();
    let effects = harness
        .core
        .command(Command::DismissMissing(Direction::Output));
    assert!(!effects.contains(&Effect::Persist));
    assert!(harness.core.settings().dismissed_missing.is_empty());
}

#[test]
fn a_closed_warning_is_kept_while_the_selection_is_switched_off() {
    let mut harness = Harness::new(Settings::default(), FakeHelper::new(true));
    harness.connect();
    harness.command(Command::Choose {
        direction: Direction::Input,
        choice: headset_mic_key(),
    });
    unplug_headset_mic(&mut harness.helper);
    harness.push_devices();
    harness.command(Command::DismissMissing(Direction::Input));

    harness.command(Command::SetEnabled(false));
    harness.command(Command::SetEnabled(true));
    assert_eq!(
        harness.core.ui_state().dismissed_missing,
        vec![Direction::Input]
    );
}

#[test]
fn garbage_from_the_helper_is_survivable() {
    let mut core = Core::new(Settings::default());
    for line in [
        "",
        "{",
        "[]",
        "{\"id\":99,\"result\":\"cleared\"}",
        "{\"event\":\"nope\"}",
    ] {
        let _ = core.incoming(line);
    }
    assert!(!core.ui_state().warnings.is_empty());
}
