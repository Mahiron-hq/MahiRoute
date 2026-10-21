// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

//! Plain-Rust halves of the ADB JNI calls, so they can be tested without a JVM.

use std::net::{Ipv4Addr, SocketAddr};
use std::path::Path;

use mahiroute_adb::{AdbError, AdbKey};
use serde_json::{Value, json};

/// Short, stable name of the failure for the Kotlin side to branch on.
fn kind(error: &AdbError) -> &'static str {
    match error {
        AdbError::Tls(_) => "not_paired",
        AdbError::Pairing(_) => "wrong_code",
        AdbError::Protocol(text) if text.contains("wireless debugging") => "not_enabled",
        AdbError::Io(_) => "unreachable",
        _ => "other",
    }
}

fn failure(error: &AdbError) -> Value {
    json!({ "error": error.to_string(), "kind": kind(error) })
}

fn local(port: i32) -> Result<SocketAddr, Value> {
    u16::try_from(port)
        .ok()
        .filter(|port| *port != 0)
        .map(|port| SocketAddr::from((Ipv4Addr::LOCALHOST, port)))
        .ok_or_else(|| json!({ "error": format!("invalid port {port}"), "kind": "other" }))
}

/// Pairs with wireless debugging; returns `{"ok":true}` or `{"error":…,"kind":…}` as text.
pub(crate) fn pair(port: i32, code: &str, key_path: &str, peer_name: &str) -> String {
    let result = local(port).and_then(|address| {
        let key = AdbKey::load_or_create(Path::new(key_path)).map_err(|e| failure(&e))?;
        mahiroute_adb::pair(address, code, &key, peer_name).map_err(|e| failure(&e))
    });
    result
        .map_or_else(|error| error, |()| json!({ "ok": true }))
        .to_string()
}

/// Starts the helper; returns `{"port":…,"token":…}` or `{"error":…,"kind":…}` as text.
pub(crate) fn start_helper(port: i32, apk_path: &str, key_path: &str, package: &str) -> String {
    let result = local(port).and_then(|address| {
        let key = AdbKey::load_or_create(Path::new(key_path)).map_err(|e| failure(&e))?;
        mahiroute_adb::start_helper(address, &key, apk_path, package).map_err(|e| failure(&e))
    });
    result
        .map_or_else(
            |error| error,
            |endpoint| json!({ "port": endpoint.port, "token": endpoint.token }),
        )
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Value {
        serde_json::from_str(text).expect("valid json")
    }

    #[test]
    fn invalid_ports_are_rejected_before_any_network_use() {
        for port in [-1, 0, 70_000] {
            let answer = parse(&pair(port, "123456", "unused", "name"));
            assert_eq!(answer["kind"], "other", "{answer}");
            let answer = parse(&start_helper(port, "/a.apk", "unused", "a.b"));
            assert_eq!(answer["kind"], "other", "{answer}");
        }
    }

    #[test]
    fn an_unreachable_service_is_classified() {
        let directory = tempfile::tempdir().expect("temp dir");
        let key_path = directory.path().join("key.pem");
        let closed = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = i32::from(closed.local_addr().expect("address").port());
        drop(closed);
        let answer = parse(&start_helper(
            port,
            "/data/app/x/base.apk",
            key_path.to_str().expect("utf-8"),
            "io.github.mahiron_hq.mahiroute",
        ));
        assert_eq!(answer["kind"], "unreachable", "{answer}");
        assert!(key_path.exists(), "the key is created on first use");
    }

    #[test]
    fn error_kinds_cover_the_cases_the_app_distinguishes() {
        assert_eq!(kind(&AdbError::Tls("x".into())), "not_paired");
        assert_eq!(kind(&AdbError::Pairing("x".into())), "wrong_code");
        assert_eq!(
            kind(&AdbError::Protocol("turn on wireless debugging".into())),
            "not_enabled"
        );
        assert_eq!(kind(&AdbError::Protocol("other".into())), "other");
        assert_eq!(kind(&AdbError::Command("x".into())), "other");
    }
}
