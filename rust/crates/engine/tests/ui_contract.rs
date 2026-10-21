// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

//! Pins the JSON the Kotlin interface parses. The same golden file is read by the Kotlin unit
//! test `UiModelContractTest`, so a change on either side fails a test instead of the app.
//!
//! Regenerate the file after an intentional change with `UPDATE_GOLDEN=1 cargo test`.

use std::fs;
use std::path::PathBuf;

use mahiroute_core::protocol::{
    Event, RawDevice, Response, ResponseBody, StreamInfo, StreamKind, encode_line,
};
use mahiroute_core::{Choice, DeviceKey, DeviceKind, Direction, Settings, Support};
use mahiroute_engine::{Command, Core, HelperTarget};

fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../app/src/test/resources/ui_state_contract.json")
}

fn raw(port: u32, input: bool, android_type: i32, address: &str, masked: bool) -> RawDevice {
    RawDevice {
        port,
        input,
        android_type,
        address: address.to_owned(),
        name: "Test phone".to_owned(),
        masked,
    }
}

fn line(response_id: u64, body: ResponseBody) -> String {
    encode_line(&Response {
        id: response_id,
        body,
    })
    .expect("encode")
}

fn representative_state() -> String {
    let mut settings = Settings::default();
    settings.selection.input = Choice::Device(DeviceKey {
        direction: Direction::Input,
        kind: DeviceKind::BuiltinMic,
        address: "bottom".to_owned(),
    });
    settings.selection.output = Choice::Device(DeviceKey {
        direction: Direction::Output,
        kind: DeviceKind::BluetoothA2dp,
        address: "AA:BB:CC:DD:EE:FF".to_owned(),
    });
    settings.dismissed_missing.insert(Direction::Output);
    settings.set_capabilities(
        "contract/fingerprint",
        mahiroute_core::Capabilities {
            capture_preset: Support::Ignored,
            output_strategy: Support::Honored,
        },
    );

    let mut core = Core::new(settings);
    let _ = core.command(Command::Connect(HelperTarget {
        port: 40000,
        token: "0123456789abcdef".to_owned(),
    }));
    let _ = core.connected();
    let _ = core.incoming(&line(
        1,
        ResponseBody::Hello {
            version: 1,
            sdk: 33,
            fingerprint: "contract/fingerprint".to_owned(),
            uid: 2000,
        },
    ));
    let _ = core.incoming(&line(
        2,
        ResponseBody::Devices {
            devices: vec![
                raw(16, true, 15, "bottom", false),
                raw(17, true, 15, "back", false),
                raw(0x8000_0000, true, 3, "", true),
                raw(3, false, 2, "", false),
                raw(1268, false, 3, "", false),
                raw(13, false, 18, "", false),
            ],
        },
    ));
    for event in [
        Event::Mode { mode: 3 },
        Event::Streams {
            streams: vec![
                StreamInfo {
                    kind: StreamKind::Record,
                    uid: 10_169,
                    source: 7,
                    device_port: Some(16),
                    device_type: Some(15),
                },
                StreamInfo {
                    kind: StreamKind::Playback,
                    uid: 10_201,
                    source: 2,
                    device_port: None,
                    device_type: None,
                },
            ],
        },
    ] {
        let _ = core.incoming(&encode_line(&event).expect("encode"));
    }
    // A response nobody asked for becomes a visible, deterministic warning.
    let _ = core.incoming(&line(99, ResponseBody::Cleared));
    serde_json::to_string_pretty(&core.ui_state()).expect("serialize")
}

#[test]
fn ui_state_json_matches_the_golden_file() {
    let actual = representative_state();
    let path = golden_path();
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        fs::create_dir_all(path.parent().expect("parent")).expect("create directory");
        fs::write(&path, format!("{actual}\n")).expect("write golden file");
    }
    let expected =
        fs::read_to_string(&path).expect("golden file exists; run with UPDATE_GOLDEN=1 once");
    assert_eq!(
        actual.trim_end(),
        expected.trim_end(),
        "the UI state JSON changed"
    );
}
