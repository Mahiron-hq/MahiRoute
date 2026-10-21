// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

//! Pure state machine of the engine: it consumes commands, helper messages and timer ticks and
//! returns the effects (requests to send, connections to open, timers) the driver must run.

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::mem;
use std::time::Duration;

use mahiroute_core::protocol::{
    Event, Incoming, PROTOCOL_VERSION, Request, RequestBody, Response, ResponseBody, StreamInfo,
};
use mahiroute_core::{
    Capabilities, Choice, DesiredRouting, Device, DeviceRef, Direction, PlanOutcome, Selection,
    Settings, Support, plan_route,
};

use crate::state::{LinkView, RoutingStatus, UiState};

/// Interval between liveness checks of an idle helper.
pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(20);
/// How long the helper may take to answer a liveness check.
pub const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(8);

const MAX_WARNINGS: usize = 12;
const RECONNECT_DELAYS_SECS: [u64; 6] = [1, 2, 4, 8, 15, 30];

const AUDIO_SOURCE_MIC: i32 = 1;
const AUDIO_SOURCE_VOICE_COMMUNICATION: i32 = 7;
const USAGE_MEDIA: i32 = 1;
const USAGE_VOICE_COMMUNICATION: i32 = 2;

/// Where the privileged helper listens.
#[derive(Clone, PartialEq, Eq)]
pub struct HelperTarget {
    /// Loopback TCP port.
    pub port: u16,
    /// Session secret the helper expects in the hello message.
    pub token: String,
}

impl fmt::Debug for HelperTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HelperTarget")
            .field("port", &self.port)
            .field("token", &"<redacted>")
            .finish()
    }
}

/// Requests from the user interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Turn the whole mechanism on or off.
    SetEnabled(bool),
    /// Pick a device, or the system default, for one direction.
    Choose {
        /// Direction being configured.
        direction: Direction,
        /// The decision.
        choice: Choice,
    },
    /// Hide the warning about a missing device until it returns or the choice changes.
    DismissMissing(Direction),
    /// Use this helper endpoint (replaces any previous one).
    Connect(HelperTarget),
    /// Forget the helper endpoint and drop the connection.
    Disconnect,
    /// Forget what is known about the firmware and measure again.
    Recalibrate,
}

/// Timers the driver keeps for the core.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Timer {
    /// Retry the helper connection.
    Reconnect,
    /// Time for the next liveness check.
    Heartbeat,
    /// The liveness check went unanswered for too long.
    HeartbeatDeadline,
}

/// Work for the driver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Open a connection; report back with `connected` or `connect_failed`.
    Connect(HelperTarget),
    /// Write a request to the helper.
    Send(Request),
    /// Close the helper connection.
    Close,
    /// Arm (or re-arm) a timer.
    Schedule(Timer, Duration),
    /// Disarm a timer.
    Cancel(Timer),
    /// Write the settings to disk.
    Persist,
    /// Publish a fresh [`UiState`].
    Emit,
}

enum Link {
    Disconnected,
    Connecting,
    Handshaking,
    Ready { sdk: u32, fingerprint: String },
    Failed(String),
}

enum Pending {
    Hello,
    Devices,
    Apply(DesiredRouting),
    Probe,
    Heartbeat,
}

struct ProbeRun {
    queue: VecDeque<(Direction, i32)>,
    current: Option<Direction>,
    expect_input: Option<DeviceRef>,
    expect_output: Option<DeviceRef>,
    results: Vec<(Direction, Option<(i32, String)>)>,
}

enum Verification {
    Idle,
    Running(ProbeRun),
    Verified,
    Unverified,
    Failed(Direction),
}

/// The engine's brain.
pub struct Core {
    settings: Settings,
    target: Option<HelperTarget>,
    link: Link,
    next_id: u64,
    pending: HashMap<u64, Pending>,
    devices: Vec<Device>,
    streams: Vec<StreamInfo>,
    mode: i32,
    capabilities: Capabilities,
    applied: Option<DesiredRouting>,
    apply_in_flight: bool,
    dirty: bool,
    outcome: PlanOutcome,
    verification: Verification,
    heartbeat_outstanding: bool,
    reconnect_attempt: usize,
    warnings: Vec<String>,
}

impl Core {
    /// Creates a core around the persisted settings.
    #[must_use]
    pub fn new(settings: Settings) -> Self {
        Self {
            settings,
            target: None,
            link: Link::Disconnected,
            next_id: 1,
            pending: HashMap::new(),
            devices: Vec::new(),
            streams: Vec::new(),
            mode: 0,
            capabilities: Capabilities::default(),
            applied: None,
            apply_in_flight: false,
            dirty: false,
            outcome: PlanOutcome::default(),
            verification: Verification::Idle,
            heartbeat_outstanding: false,
            reconnect_attempt: 0,
            warnings: Vec::new(),
        }
    }

    /// The settings to persist after an [`Effect::Persist`].
    #[must_use]
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Records a problem that the user may want to see.
    pub fn warn(&mut self, message: String) {
        if self.warnings.len() == MAX_WARNINGS {
            self.warnings.remove(0);
        }
        self.warnings.push(message);
    }

    /// Handles a request from the user interface.
    pub fn command(&mut self, command: Command) -> Vec<Effect> {
        let mut fx = Vec::new();
        match command {
            Command::SetEnabled(enabled) => {
                self.settings.selection.enabled = enabled;
                fx.push(Effect::Persist);
                self.reconcile(&mut fx);
            }
            Command::Choose { direction, choice } => {
                match direction {
                    Direction::Input => self.settings.selection.input = choice,
                    Direction::Output => self.settings.selection.output = choice,
                }
                self.settings.dismissed_missing.remove(&direction);
                fx.push(Effect::Persist);
                self.reconcile(&mut fx);
            }
            Command::DismissMissing(direction) => {
                if self.outcome.missing.contains(&direction)
                    && self.settings.dismissed_missing.insert(direction)
                {
                    fx.push(Effect::Persist);
                }
            }
            Command::Connect(target) => {
                if !matches!(self.link, Link::Disconnected | Link::Failed(_)) {
                    fx.push(Effect::Close);
                    self.reset_session();
                }
                self.target = Some(target.clone());
                self.reconnect_attempt = 0;
                self.link = Link::Connecting;
                fx.push(Effect::Cancel(Timer::Reconnect));
                fx.push(Effect::Connect(target));
            }
            Command::Disconnect => {
                self.target = None;
                self.reset_session();
                self.link = Link::Disconnected;
                fx.extend([
                    Effect::Close,
                    Effect::Cancel(Timer::Reconnect),
                    Effect::Cancel(Timer::Heartbeat),
                    Effect::Cancel(Timer::HeartbeatDeadline),
                ]);
            }
            Command::Recalibrate => {
                if let Link::Ready { fingerprint, .. } = &self.link {
                    self.capabilities = Capabilities::default();
                    let fingerprint = fingerprint.clone();
                    self.settings
                        .set_capabilities(&fingerprint, self.capabilities);
                    fx.push(Effect::Persist);
                    self.applied = None;
                    self.reconcile(&mut fx);
                }
            }
        }
        fx.push(Effect::Emit);
        fx
    }

    /// The transport to the helper is open: introduce ourselves.
    pub fn connected(&mut self) -> Vec<Effect> {
        let Some(target) = self.target.clone() else {
            return vec![Effect::Close];
        };
        self.link = Link::Handshaking;
        let mut fx = Vec::new();
        self.send(
            RequestBody::Hello {
                token: target.token,
                version: PROTOCOL_VERSION,
            },
            Pending::Hello,
            &mut fx,
        );
        fx.push(Effect::Emit);
        fx
    }

    /// Opening the transport failed.
    pub fn connect_failed(&mut self, reason: &str) -> Vec<Effect> {
        self.closed(reason)
    }

    /// The transport broke or the helper hung up.
    pub fn closed(&mut self, reason: &str) -> Vec<Effect> {
        let mut fx = vec![
            Effect::Close,
            Effect::Cancel(Timer::Heartbeat),
            Effect::Cancel(Timer::HeartbeatDeadline),
        ];
        self.reset_session();
        self.link = Link::Failed(reason.to_owned());
        if self.target.is_some() {
            let delay =
                RECONNECT_DELAYS_SECS[self.reconnect_attempt.min(RECONNECT_DELAYS_SECS.len() - 1)];
            self.reconnect_attempt += 1;
            fx.push(Effect::Schedule(
                Timer::Reconnect,
                Duration::from_secs(delay),
            ));
        }
        fx.push(Effect::Emit);
        fx
    }

    /// A timer set through [`Effect::Schedule`] fired.
    pub fn timer(&mut self, timer: Timer) -> Vec<Effect> {
        let mut fx = Vec::new();
        match timer {
            Timer::Reconnect => {
                if let Some(target) = self.target.clone() {
                    self.link = Link::Connecting;
                    fx.push(Effect::Connect(target));
                    fx.push(Effect::Emit);
                }
            }
            Timer::Heartbeat => {
                if matches!(self.link, Link::Ready { .. }) {
                    self.heartbeat_outstanding = true;
                    self.send(RequestBody::ListDevices, Pending::Heartbeat, &mut fx);
                    fx.push(Effect::Schedule(
                        Timer::HeartbeatDeadline,
                        HEARTBEAT_TIMEOUT,
                    ));
                }
            }
            Timer::HeartbeatDeadline => {
                if self.heartbeat_outstanding {
                    return self.closed("the helper stopped answering");
                }
            }
        }
        fx
    }

    /// A line arrived from the helper.
    pub fn incoming(&mut self, line: &str) -> Vec<Effect> {
        let mut fx = Vec::new();
        match Incoming::parse(line) {
            Ok(Incoming::Response(response)) => self.on_response(response, &mut fx),
            Ok(Incoming::Event(event)) => self.on_event(event, &mut fx),
            Err(error) => self.warn(format!("unreadable helper message: {error}")),
        }
        fx.push(Effect::Emit);
        fx
    }

    /// Snapshot for the user interface.
    #[must_use]
    pub fn ui_state(&self) -> UiState {
        let (link, sdk) = match &self.link {
            Link::Disconnected => (LinkView::Disconnected, None),
            Link::Connecting | Link::Handshaking => (LinkView::Connecting, None),
            Link::Ready { sdk, .. } => (LinkView::Ready, Some(*sdk)),
            Link::Failed(reason) => (
                LinkView::Failed {
                    reason: reason.clone(),
                },
                None,
            ),
        };
        let mut devices: Vec<Device> = self
            .devices
            .iter()
            .filter(|device| device.kind.is_user_selectable())
            .cloned()
            .collect();
        devices.sort_by(|a, b| {
            (a.direction, a.kind, &a.address, a.port).cmp(&(
                b.direction,
                b.kind,
                &b.address,
                b.port,
            ))
        });
        UiState {
            link,
            sdk,
            enabled: self.settings.selection.enabled,
            selection: self.settings.selection.clone(),
            devices,
            missing: self.outcome.missing.clone(),
            dismissed_missing: self
                .settings
                .dismissed_missing
                .iter()
                .copied()
                .filter(|direction| self.outcome.missing.contains(direction))
                .collect(),
            routing: self.routing_status(),
            capabilities: self.capabilities,
            streams: self.streams.clone(),
            mode: self.mode,
            warnings: self.warnings.clone(),
        }
    }

    fn routing_status(&self) -> RoutingStatus {
        let overriding = self.outcome.desired.prefer_input.is_some()
            || self.outcome.desired.prefer_output.is_some();
        if !overriding || !matches!(self.link, Link::Ready { .. }) {
            return RoutingStatus::Inactive;
        }
        match &self.verification {
            Verification::Running(_) => RoutingStatus::Verifying,
            Verification::Verified => RoutingStatus::Verified,
            Verification::Unverified => RoutingStatus::Unverified,
            Verification::Failed(direction) => RoutingStatus::Failed {
                direction: *direction,
            },
            Verification::Idle => RoutingStatus::Applying,
        }
    }

    fn reset_session(&mut self) {
        self.pending.clear();
        self.applied = None;
        self.apply_in_flight = false;
        self.dirty = false;
        self.verification = Verification::Idle;
        self.heartbeat_outstanding = false;
    }

    fn send(&mut self, body: RequestBody, pending: Pending, fx: &mut Vec<Effect>) {
        let id = self.next_id;
        self.next_id += 1;
        self.pending.insert(id, pending);
        fx.push(Effect::Send(Request { id, body }));
    }

    fn on_response(&mut self, response: Response, fx: &mut Vec<Effect>) {
        let Some(pending) = self.pending.remove(&response.id) else {
            self.warn(format!("unexpected helper response {}", response.id));
            return;
        };
        match (pending, response.body) {
            (_, ResponseBody::Error { message }) => self.on_error(&message, fx),
            (
                Pending::Hello,
                ResponseBody::Hello {
                    sdk, fingerprint, ..
                },
            ) => {
                self.capabilities = self.settings.capabilities_for(&fingerprint);
                self.link = Link::Ready { sdk, fingerprint };
                self.reconnect_attempt = 0;
                self.send(RequestBody::ListDevices, Pending::Devices, fx);
                fx.push(Effect::Schedule(Timer::Heartbeat, HEARTBEAT_INTERVAL));
            }
            (Pending::Devices, ResponseBody::Devices { devices }) => {
                self.set_devices(devices);
                self.reconcile(fx);
            }
            (Pending::Heartbeat, ResponseBody::Devices { devices }) => {
                self.heartbeat_outstanding = false;
                fx.push(Effect::Cancel(Timer::HeartbeatDeadline));
                fx.push(Effect::Schedule(Timer::Heartbeat, HEARTBEAT_INTERVAL));
                self.set_devices(devices);
                self.reconcile(fx);
            }
            (Pending::Apply(desired), ResponseBody::Applied { warnings }) => {
                for warning in warnings {
                    self.warn(warning);
                }
                self.on_applied(desired, fx);
            }
            (
                Pending::Probe,
                ResponseBody::Probed {
                    routed_type,
                    routed_address,
                    ..
                },
            ) => {
                let routed = routed_type.map(|kind| (kind, routed_address.unwrap_or_default()));
                self.on_probed(routed, fx);
            }
            (_, other) => self.warn(format!("helper answered out of turn: {other:?}")),
        }
    }

    fn on_error(&mut self, message: &str, fx: &mut Vec<Effect>) {
        self.warn(format!("helper error: {message}"));
        if matches!(self.link, Link::Handshaking) {
            fx.extend(self.closed("the helper rejected the connection"));
            return;
        }
        // A failed apply or probe must not wedge the state machine.
        self.apply_in_flight = false;
        if matches!(self.verification, Verification::Running(_)) {
            self.verification = Verification::Unverified;
        }
    }

    fn on_event(&mut self, event: Event, fx: &mut Vec<Effect>) {
        match event {
            Event::Devices { devices } => {
                self.set_devices(devices);
                self.reconcile(fx);
            }
            Event::Mode { mode } => self.mode = mode,
            Event::Streams { streams } => self.streams = streams,
        }
    }

    fn set_devices(&mut self, raw: Vec<mahiroute_core::protocol::RawDevice>) {
        self.devices = raw.into_iter().map(Device::from).collect();
    }

    /// Re-plans and sends an apply request when the desired state changed.
    fn reconcile(&mut self, fx: &mut Vec<Effect>) {
        if !matches!(self.link, Link::Ready { .. }) {
            self.outcome = PlanOutcome::default();
            return;
        }
        self.outcome = plan_route(&self.settings.selection, &self.devices, &self.capabilities);
        if self.forget_resolved_dismissals() {
            fx.push(Effect::Persist);
        }
        if self.apply_in_flight {
            self.dirty = true;
            return;
        }
        if self.applied.as_ref() != Some(&self.outcome.desired) {
            let desired = self.outcome.desired.clone();
            self.apply_in_flight = true;
            self.send(
                RequestBody::Apply {
                    desired: desired.clone(),
                },
                Pending::Apply(desired),
                fx,
            );
        }
    }

    /// Drops acknowledged warnings that no longer apply: the choice went back to the system
    /// default, or the chosen device is connected again. Returns whether anything changed.
    fn forget_resolved_dismissals(&mut self) -> bool {
        let Settings {
            selection,
            dismissed_missing,
            ..
        } = &mut self.settings;
        let missing = &self.outcome.missing;
        let before = dismissed_missing.len();
        dismissed_missing.retain(|direction| {
            let choice = match direction {
                Direction::Input => &selection.input,
                Direction::Output => &selection.output,
            };
            // A disabled selection plans nothing, so absence cannot be judged then.
            matches!(choice, Choice::Device(_))
                && (!selection.enabled || missing.contains(direction))
        });
        dismissed_missing.len() != before
    }

    fn on_applied(&mut self, desired: DesiredRouting, fx: &mut Vec<Effect>) {
        self.apply_in_flight = false;
        self.applied = Some(desired);
        if self.dirty {
            self.dirty = false;
            self.reconcile(fx);
            if self.apply_in_flight {
                return;
            }
        }
        self.start_verification(fx);
    }

    fn start_verification(&mut self, fx: &mut Vec<Effect>) {
        let desired = self.outcome.desired.clone();
        let mut queue = VecDeque::new();
        if desired.prefer_input.is_some() {
            queue.push_back((Direction::Input, AUDIO_SOURCE_MIC));
            queue.push_back((Direction::Input, AUDIO_SOURCE_VOICE_COMMUNICATION));
        }
        if desired.prefer_output.is_some() {
            queue.push_back((Direction::Output, USAGE_MEDIA));
            queue.push_back((Direction::Output, USAGE_VOICE_COMMUNICATION));
        }
        if queue.is_empty() {
            self.verification = Verification::Idle;
            return;
        }
        self.verification = Verification::Running(ProbeRun {
            queue,
            current: None,
            expect_input: desired.prefer_input,
            expect_output: desired.prefer_output,
            results: Vec::new(),
        });
        self.next_probe(fx);
    }

    fn next_probe(&mut self, fx: &mut Vec<Effect>) {
        let Verification::Running(run) = &mut self.verification else {
            return;
        };
        let Some((direction, kind)) = run.queue.pop_front() else {
            self.finish_verification(fx);
            return;
        };
        run.current = Some(direction);
        self.send(RequestBody::Probe { direction, kind }, Pending::Probe, fx);
    }

    fn on_probed(&mut self, routed: Option<(i32, String)>, fx: &mut Vec<Effect>) {
        if let Verification::Running(run) = &mut self.verification {
            if let Some(direction) = run.current.take() {
                run.results.push((direction, routed));
            }
            self.next_probe(fx);
        }
    }

    fn finish_verification(&mut self, fx: &mut Vec<Effect>) {
        let Verification::Running(run) = mem::replace(&mut self.verification, Verification::Idle)
        else {
            return;
        };
        let mut mismatched = Vec::new();
        let mut inconclusive = false;
        for (direction, expected) in [
            (Direction::Input, &run.expect_input),
            (Direction::Output, &run.expect_output),
        ] {
            let Some(expected) = expected else { continue };
            let probes: Vec<_> = run
                .results
                .iter()
                .filter(|result| result.0 == direction)
                .collect();
            if probes.is_empty() || probes.iter().any(|result| result.1.is_none()) {
                inconclusive = true;
                continue;
            }
            let wanted = (expected.android_type, expected.address.clone());
            if probes
                .iter()
                .all(|result| result.1.as_ref() == Some(&wanted))
            {
                if self.support(direction) == Support::Unknown {
                    self.set_support(direction, Support::Honored);
                    fx.push(Effect::Persist);
                }
            } else {
                mismatched.push(direction);
            }
        }
        if mismatched.is_empty() {
            self.verification = if inconclusive {
                Verification::Unverified
            } else {
                Verification::Verified
            };
            return;
        }
        let mut escalated = false;
        for direction in &mismatched {
            if self.support(*direction).preference_suffices() {
                self.set_support(*direction, Support::Ignored);
                escalated = true;
            }
        }
        if escalated {
            fx.push(Effect::Persist);
            self.reconcile(fx);
        }
        if !escalated || !self.apply_in_flight {
            self.warn("the audio system did not follow the selected device".to_owned());
            self.verification = Verification::Failed(mismatched[0]);
        }
    }

    fn support(&self, direction: Direction) -> Support {
        match direction {
            Direction::Input => self.capabilities.capture_preset,
            Direction::Output => self.capabilities.output_strategy,
        }
    }

    fn set_support(&mut self, direction: Direction, support: Support) {
        match direction {
            Direction::Input => self.capabilities.capture_preset = support,
            Direction::Output => self.capabilities.output_strategy = support,
        }
        if let Link::Ready { fingerprint, .. } = &self.link {
            let fingerprint = fingerprint.clone();
            self.settings
                .set_capabilities(&fingerprint, self.capabilities);
        }
    }

    /// The master selection as currently stored.
    #[must_use]
    pub fn selection(&self) -> &Selection {
        &self.settings.selection
    }
}
