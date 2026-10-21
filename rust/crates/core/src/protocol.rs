// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

//! Wire protocol between the app and the shell-privileged helper.
//!
//! Both sides exchange UTF-8 JSON objects separated by `\n`. The app sends [`Request`]s; the
//! helper answers each with a [`Response`] carrying the same `id` and may push [`Event`]s at any
//! time.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::device::{Device, DeviceKind, Direction, PortId};
use crate::plan::DesiredRouting;

/// Version of this protocol; bumped on every incompatible change.
pub const PROTOCOL_VERSION: u32 = 1;

/// Longest accepted line, so a misbehaving peer cannot exhaust memory.
pub const MAX_LINE_BYTES: usize = 1 << 20;

/// Failure to encode or decode a protocol line.
#[derive(Debug, Error)]
pub enum ProtocolError {
    /// The line is not valid JSON or does not match any message.
    #[error("malformed protocol line: {0}")]
    Malformed(#[from] serde_json::Error),
    /// The line exceeds [`MAX_LINE_BYTES`].
    #[error("protocol line of {0} bytes exceeds the limit")]
    TooLong(usize),
    /// The line is a JSON value but not an object.
    #[error("protocol line is not a JSON object")]
    NotAnObject,
}

/// A device as the helper reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawDevice {
    /// Port identifier.
    pub port: u32,
    /// True for microphones, false for speakers.
    pub input: bool,
    /// Raw `AudioDeviceInfo.TYPE_*` value.
    #[serde(rename = "type")]
    pub android_type: i32,
    /// Hardware address.
    pub address: String,
    /// Product name.
    pub name: String,
    /// The helper currently hides this device from the audio policy.
    #[serde(default)]
    pub masked: bool,
}

impl From<RawDevice> for Device {
    fn from(raw: RawDevice) -> Self {
        Self {
            port: PortId(raw.port),
            direction: if raw.input {
                Direction::Input
            } else {
                Direction::Output
            },
            kind: DeviceKind::from_android_type(raw.android_type),
            android_type: raw.android_type,
            address: raw.address,
            name: raw.name,
            masked: raw.masked,
        }
    }
}

/// Whether a stream records or plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamKind {
    /// An active recording session.
    Record,
    /// An active playback session.
    Playback,
}

/// One active audio stream of some app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamInfo {
    /// Recording or playback.
    pub kind: StreamKind,
    /// Linux uid of the owning app.
    pub uid: i32,
    /// Audio source for recordings, usage for playback.
    pub source: i32,
    /// Port the stream is routed to, when known.
    pub device_port: Option<u32>,
    /// Raw type of the routed device, when known.
    pub device_type: Option<i32>,
}

/// Operations the app can ask the helper to perform.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum RequestBody {
    /// First message on a connection; proves knowledge of the session token.
    Hello {
        /// Token the app passed to the helper on its command line.
        token: String,
        /// Protocol version the app speaks.
        version: u32,
    },
    /// Return the connected devices, including masked ones.
    ListDevices,
    /// Converge to the given routing state.
    Apply {
        /// The state to reach.
        desired: DesiredRouting,
    },
    /// Remove every preference and unmask every device.
    ClearAll,
    /// Return the active recording and playback streams.
    Diagnostics,
    /// Open a short silent stream and report the device the system routed it to.
    Probe {
        /// `Input` records, `Output` plays.
        direction: Direction,
        /// Audio source for recording probes, usage for playback probes.
        kind: i32,
    },
    /// Restore the system and stop the helper.
    Shutdown,
}

/// A request with its correlation id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    /// Correlation id echoed in the response.
    pub id: u64,
    /// The operation.
    #[serde(flatten)]
    pub body: RequestBody,
}

/// Payload of a helper response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum ResponseBody {
    /// Answer to [`RequestBody::Hello`].
    Hello {
        /// Protocol version the helper speaks.
        version: u32,
        /// `Build.VERSION.SDK_INT` of the device.
        sdk: u32,
        /// `Build.FINGERPRINT` of the firmware, used to key probe results.
        fingerprint: String,
        /// Linux uid the helper runs as (2000 for shell).
        uid: u32,
    },
    /// Answer to [`RequestBody::ListDevices`].
    Devices {
        /// Connected devices.
        devices: Vec<RawDevice>,
    },
    /// Answer to [`RequestBody::Apply`].
    Applied {
        /// Non-fatal problems met while applying.
        warnings: Vec<String>,
    },
    /// Answer to [`RequestBody::ClearAll`].
    Cleared,
    /// Answer to [`RequestBody::Diagnostics`].
    Diagnostics {
        /// Active streams.
        streams: Vec<StreamInfo>,
    },
    /// Answer to [`RequestBody::Probe`].
    Probed {
        /// Port the probe stream was routed to, when the system reported one.
        routed_port: Option<u32>,
        /// Raw type of the routed device.
        routed_type: Option<i32>,
        /// Address of the routed device.
        routed_address: Option<String>,
        /// Root-mean-square level of the captured samples (recording probes only).
        level: Option<u32>,
    },
    /// The request failed.
    Error {
        /// Human-readable cause.
        message: String,
    },
}

/// A response with its correlation id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Response {
    /// Id of the request this answers.
    pub id: u64,
    /// The payload.
    #[serde(flatten)]
    pub body: ResponseBody,
}

/// Unsolicited notifications from the helper.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    /// The set of connected devices changed.
    Devices {
        /// The new set of devices.
        devices: Vec<RawDevice>,
    },
    /// The audio mode (normal, in call, in communication) changed.
    Mode {
        /// `AudioManager.MODE_*` value.
        mode: i32,
    },
    /// The set of active streams changed.
    Streams {
        /// Active streams.
        streams: Vec<StreamInfo>,
    },
}

/// A line received from the helper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Incoming {
    /// Answer to an earlier request.
    Response(Response),
    /// Unsolicited notification.
    Event(Event),
}

impl Incoming {
    /// Parses one line (without its terminator) received from the helper.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError`] when the line is too long, not a JSON object, or matches no
    /// known response or event.
    pub fn parse(line: &str) -> Result<Self, ProtocolError> {
        if line.len() > MAX_LINE_BYTES {
            return Err(ProtocolError::TooLong(line.len()));
        }
        let value: Value = serde_json::from_str(line)?;
        let object = value.as_object().ok_or(ProtocolError::NotAnObject)?;
        if object.contains_key("id") {
            Ok(Self::Response(serde_json::from_value(value)?))
        } else {
            Ok(Self::Event(serde_json::from_value(value)?))
        }
    }
}

/// Serializes a message and appends the line terminator.
///
/// # Errors
///
/// Returns [`ProtocolError::Malformed`] if serialization fails and
/// [`ProtocolError::TooLong`] if the encoded line exceeds [`MAX_LINE_BYTES`].
pub fn encode_line<T: Serialize>(message: &T) -> Result<String, ProtocolError> {
    let mut line = serde_json::to_string(message)?;
    if line.len() > MAX_LINE_BYTES {
        return Err(ProtocolError::TooLong(line.len()));
    }
    line.push('\n');
    Ok(line)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::DeviceRef;
    use proptest::prelude::*;

    #[test]
    fn request_encodes_as_flat_json_line() {
        let request = Request {
            id: 3,
            body: RequestBody::Hello {
                token: "t".to_owned(),
                version: PROTOCOL_VERSION,
            },
        };
        let line = encode_line(&request).expect("encode");
        assert_eq!(
            line,
            "{\"id\":3,\"op\":\"hello\",\"token\":\"t\",\"version\":1}\n"
        );
    }

    #[test]
    fn apply_request_round_trips() {
        let request = Request {
            id: 9,
            body: RequestBody::Apply {
                desired: DesiredRouting {
                    prefer_input: Some(DeviceRef {
                        direction: Direction::Input,
                        android_type: 15,
                        address: "bottom".to_owned(),
                    }),
                    prefer_output: None,
                    masked: vec![],
                },
            },
        };
        let line = encode_line(&request).expect("encode");
        let decoded: Request = serde_json::from_str(line.trim_end()).expect("decode");
        assert_eq!(decoded, request);
    }

    #[test]
    fn probe_round_trips() {
        let request = Request {
            id: 5,
            body: RequestBody::Probe {
                direction: Direction::Input,
                kind: 7,
            },
        };
        let line = encode_line(&request).expect("encode");
        assert_eq!(
            line,
            "{\"id\":5,\"op\":\"probe\",\"direction\":\"input\",\"kind\":7}\n"
        );
        let answer = r#"{"id":5,"result":"probed","routed_port":16,"routed_type":15,"routed_address":"bottom","level":42}"#;
        assert_eq!(
            Incoming::parse(answer).expect("response"),
            Incoming::Response(Response {
                id: 5,
                body: ResponseBody::Probed {
                    routed_port: Some(16),
                    routed_type: Some(15),
                    routed_address: Some("bottom".to_owned()),
                    level: Some(42),
                }
            })
        );
        let silent = r#"{"id":6,"result":"probed","routed_port":null,"routed_type":null,"routed_address":null,"level":null}"#;
        assert!(Incoming::parse(silent).is_ok());
    }

    #[test]
    fn parses_response_and_event_lines() {
        let response = Incoming::parse(r#"{"id":4,"result":"cleared"}"#).expect("response");
        assert_eq!(
            response,
            Incoming::Response(Response {
                id: 4,
                body: ResponseBody::Cleared
            })
        );
        let event = Incoming::parse(r#"{"event":"mode","mode":3}"#).expect("event");
        assert_eq!(event, Incoming::Event(Event::Mode { mode: 3 }));
    }

    #[test]
    fn parses_device_lists_with_missing_masked_flag() {
        let line = r#"{"event":"devices","devices":[{"port":16,"input":true,"type":15,"address":"bottom","name":"M"}]}"#;
        let Incoming::Event(Event::Devices { devices }) = Incoming::parse(line).expect("event")
        else {
            panic!("expected a devices event");
        };
        let device = Device::from(devices[0].clone());
        assert_eq!(device.kind, DeviceKind::BuiltinMic);
        assert_eq!(device.direction, Direction::Input);
        assert!(!device.masked);
    }

    #[test]
    fn rejects_garbage_and_oversized_lines() {
        assert!(matches!(
            Incoming::parse("not json"),
            Err(ProtocolError::Malformed(_))
        ));
        assert!(matches!(
            Incoming::parse("[1,2]"),
            Err(ProtocolError::NotAnObject)
        ));
        assert!(matches!(
            Incoming::parse(r#"{"event":"nope"}"#),
            Err(ProtocolError::Malformed(_))
        ));
        let long = format!(
            "{{\"event\":\"mode\",\"mode\":0,\"pad\":\"{}\"}}",
            "x".repeat(MAX_LINE_BYTES)
        );
        assert!(matches!(
            Incoming::parse(&long),
            Err(ProtocolError::TooLong(_))
        ));
    }

    proptest! {
        #[test]
        fn raw_device_round_trips(
            port in any::<u32>(),
            input in any::<bool>(),
            android_type in any::<i32>(),
            address in "\\PC{0,20}",
            name in "\\PC{0,20}",
            masked in any::<bool>(),
        ) {
            let raw = RawDevice { port, input, android_type, address, name, masked };
            let event = Event::Devices { devices: vec![raw.clone()] };
            let line = encode_line(&event).expect("encode");
            let Incoming::Event(Event::Devices { devices }) = Incoming::parse(line.trim_end()).expect("decode") else {
                panic!("expected devices event");
            };
            prop_assert_eq!(&devices[0], &raw);
        }

        #[test]
        fn parse_never_panics(line in "\\PC{0,200}") {
            let _ = Incoming::parse(&line);
        }
    }
}
