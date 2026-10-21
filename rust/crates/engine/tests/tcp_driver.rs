// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

//! The threaded engine talking to a helper stand-in over real loopback sockets.

use std::io::{BufReader, Write};
use std::net::TcpListener;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use mahiroute_core::protocol::{
    RawDevice, Request, RequestBody, Response, ResponseBody, encode_line,
};
use mahiroute_core::{Choice, DeviceKey, DeviceKind, Direction};
use mahiroute_engine::io::read_bounded_line;
use mahiroute_engine::{
    Command, Engine, HelperTarget, LinkView, Listener, RoutingStatus, TcpConnector, UiState,
};

#[derive(Default)]
struct Collector {
    states: Mutex<Vec<UiState>>,
    changed: Condvar,
}

impl Listener for Collector {
    fn state_changed(&self, state: &UiState) {
        self.states.lock().expect("lock").push(state.clone());
        self.changed.notify_all();
    }
}

impl Collector {
    fn wait_for(&self, what: &str, predicate: impl Fn(&UiState) -> bool) -> UiState {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut states = self.states.lock().expect("lock");
        loop {
            if let Some(found) = states.iter().rev().find(|state| predicate(state)) {
                return found.clone();
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            assert!(!remaining.is_zero(), "timed out waiting for {what}");
            states = self
                .changed
                .wait_timeout(states, remaining)
                .expect("wait")
                .0;
        }
    }
}

fn device(port: u32, input: bool, android_type: i32, address: &str) -> RawDevice {
    RawDevice {
        port,
        input,
        android_type,
        address: address.to_owned(),
        name: "stand-in".to_owned(),
        masked: false,
    }
}

/// Serves one connection like a firmware that honors preferences, then hangs up.
fn serve_once(listener: &TcpListener, requests_before_hangup: usize) {
    let (stream, _) = listener.accept().expect("accept");
    let mut reader = BufReader::new(stream.try_clone().expect("clone"));
    let mut writer = stream;
    let mut preferred: Option<(i32, String)> = None;
    for _ in 0..requests_before_hangup {
        let Some(line) = read_bounded_line(&mut reader).expect("read") else {
            return;
        };
        let request: Request = serde_json::from_str(&line).expect("request");
        let body = match request.body {
            RequestBody::Hello { .. } => ResponseBody::Hello {
                version: 1,
                sdk: 36,
                fingerprint: "stand-in/1".to_owned(),
                uid: 2000,
            },
            RequestBody::ListDevices => ResponseBody::Devices {
                devices: vec![
                    device(16, true, 15, "bottom"),
                    device(1270, true, 3, ""),
                    device(3, false, 2, ""),
                ],
            },
            RequestBody::Apply { desired } => {
                preferred = desired
                    .prefer_input
                    .map(|input| (input.android_type, input.address));
                ResponseBody::Applied {
                    warnings: Vec::new(),
                }
            }
            RequestBody::Probe { .. } => {
                let (kind, address) = preferred.clone().unwrap_or((3, String::new()));
                ResponseBody::Probed {
                    routed_port: Some(16),
                    routed_type: Some(kind),
                    routed_address: Some(address),
                    level: None,
                }
            }
            RequestBody::ClearAll | RequestBody::Diagnostics | RequestBody::Shutdown => {
                ResponseBody::Cleared
            }
        };
        let line = encode_line(&Response {
            id: request.id,
            body,
        })
        .expect("encode");
        writer.write_all(line.as_bytes()).expect("write");
    }
}

#[test]
fn engine_connects_applies_verifies_and_notices_a_hangup() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("address").port();
    // hello, devices, apply, two input probes: then the stand-in disappears.
    let helper = thread::spawn(move || serve_once(&listener, 5));

    let directory = tempfile::tempdir().expect("temp dir");
    let collector = Arc::new(Collector::default());
    let engine = Engine::start(
        directory.path().join("settings.json"),
        Arc::new(TcpConnector),
        Arc::clone(&collector) as Arc<dyn Listener>,
    );
    engine.command(Command::Choose {
        direction: Direction::Input,
        choice: Choice::Device(DeviceKey {
            direction: Direction::Input,
            kind: DeviceKind::BuiltinMic,
            address: "bottom".to_owned(),
        }),
    });
    engine.command(Command::Connect(HelperTarget {
        port,
        token: "0123456789abcdef".to_owned(),
    }));

    let ready = collector.wait_for("verified routing", |state| {
        state.routing == RoutingStatus::Verified
    });
    assert_eq!(ready.link, LinkView::Ready);
    assert_eq!(ready.sdk, Some(36));
    assert!(
        ready
            .devices
            .iter()
            .any(|device| device.kind == DeviceKind::BuiltinMic)
    );

    helper.join().expect("helper thread");
    let failed = collector.wait_for("the hangup", |state| {
        matches!(state.link, LinkView::Failed { .. })
    });
    assert_eq!(failed.routing, RoutingStatus::Inactive);

    drop(engine);
    let saved = std::fs::read_to_string(directory.path().join("settings.json"))
        .expect("settings persisted");
    assert!(saved.contains("builtin_mic"));
}
