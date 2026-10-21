// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

//! Starting the shell-privileged helper of the app through an ADB shell.

use std::net::SocketAddr;
use std::path::Path;
use std::time::Duration;

use crate::error::AdbError;
use crate::key::AdbKey;
use crate::session::run_shell;

/// Class that `app_process` runs; it lives in the app's own APK.
pub const HELPER_CLASS: &str = "io.github.mahiron_hq.mahiroute.helper.HelperMain";
/// Process name of the helper, used to stop a previous instance.
pub const HELPER_PROCESS_NAME: &str = "mahiroute-helper";
/// Where the helper keeps what it changed, so a successor can undo it.
pub const HELPER_STATE_FILE: &str = "/data/local/tmp/mahiroute-state.json";
/// Where the helper writes its output while starting.
pub const HELPER_LOG_FILE: &str = "/data/local/tmp/mahiroute-helper.log";
/// Line the helper prints once it listens, followed by its port.
pub const READY_MARKER: &str = "MAHIROUTE_READY";

const START_TIMEOUT: Duration = Duration::from_secs(25);
const TOKEN_BYTES: usize = 16;

/// Where a freshly started helper listens and the secret it expects.
#[derive(Clone, PartialEq, Eq)]
pub struct HelperEndpoint {
    /// Loopback TCP port.
    pub port: u16,
    /// Session secret, lowercase hex.
    pub token: String,
}

impl std::fmt::Debug for HelperEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HelperEndpoint")
            .field("port", &self.port)
            .field("token", &"<redacted>")
            .finish()
    }
}

/// Builds the shell command that replaces any running helper with a new one and prints its port.
///
/// # Errors
///
/// Returns [`AdbError::Command`] when `apk_path`, `token` or `package` contain characters that
/// cannot be passed safely to the shell.
pub fn launch_command(apk_path: &str, token: &str, package: &str) -> Result<String, AdbError> {
    let path_ok = apk_path.starts_with('/')
        && Path::new(apk_path)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("apk"))
        && apk_path.chars().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-' | '=' | '~' | '+')
        });
    if !path_ok {
        return Err(AdbError::Command(format!("unsafe APK path {apk_path:?}")));
    }
    if token.len() < 16 || !token.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(AdbError::Command(
            "the helper token must be at least 16 hex digits".to_owned(),
        ));
    }
    if package.is_empty()
        || !package
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_'))
    {
        return Err(AdbError::Command(format!(
            "unsafe package name {package:?}"
        )));
    }
    Ok(format!(
        "pm grant {package} android.permission.WRITE_SECURE_SETTINGS 2>/dev/null; \
         kill $(pidof {HELPER_PROCESS_NAME}) 2>/dev/null; sleep 0.3; rm -f {HELPER_LOG_FILE}; \
         CLASSPATH='{apk_path}' nohup app_process /system/bin --nice-name={HELPER_PROCESS_NAME} \
         {HELPER_CLASS} --token {token} --port 0 --state {HELPER_STATE_FILE} \
         > {HELPER_LOG_FILE} 2>&1 < /dev/null & \
         i=0; while [ $i -lt 80 ]; do grep -q {READY_MARKER} {HELPER_LOG_FILE} && break; i=$((i+1)); sleep 0.25; done; \
         grep {READY_MARKER} {HELPER_LOG_FILE}"
    ))
}

/// Extracts the port from the helper's ready line in the shell output.
///
/// # Errors
///
/// Returns [`AdbError::Command`] when the helper did not report that it is listening.
pub fn parse_ready(output: &str) -> Result<u16, AdbError> {
    output
        .lines()
        .filter_map(|line| line.trim().strip_prefix(READY_MARKER))
        .find_map(|rest| rest.trim().parse::<u16>().ok())
        .ok_or_else(|| {
            AdbError::Command("the helper did not report that it is listening".to_owned())
        })
}

/// Generates a session token: 128 random bits as lowercase hex.
///
/// # Errors
///
/// Returns [`AdbError::Tls`] when the system random generator fails.
pub fn generate_token() -> Result<String, AdbError> {
    let mut bytes = [0u8; TOKEN_BYTES];
    boring::rand::rand_bytes(&mut bytes)?;
    Ok(format!("{:032x}", u128::from_be_bytes(bytes)))
}

/// Starts the helper through the wireless-debugging endpoint at `address`.
///
/// `package` is the app's own package name; the app is granted permission to change secure
/// settings on the way, so later starts can switch wireless debugging on by themselves.
///
/// # Errors
///
/// Returns the errors of [`run_shell`] and [`AdbError::Command`] when the helper never reports
/// that it is ready.
pub fn start_helper(
    address: SocketAddr,
    key: &AdbKey,
    apk_path: &str,
    package: &str,
) -> Result<HelperEndpoint, AdbError> {
    let token = generate_token()?;
    let output = run_shell(
        address,
        key,
        &launch_command(apk_path, &token, package)?,
        START_TIMEOUT,
    )?;
    Ok(HelperEndpoint {
        port: parse_ready(&output)?,
        token,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const APK: &str = "/data/app/~~AbC123==/io.github.mahiron_hq.mahiroute-xYz789==/base.apk";
    const PACKAGE: &str = "io.github.mahiron_hq.mahiroute";

    #[test]
    fn command_contains_the_pieces_the_helper_needs() {
        let command =
            launch_command(APK, "0123456789abcdef0123456789abcdef", PACKAGE).expect("command");
        assert!(command.contains(&format!("CLASSPATH='{APK}'")));
        assert!(command.contains("--token 0123456789abcdef0123456789abcdef"));
        assert!(command.contains(HELPER_CLASS));
        assert!(command.contains("--port 0"));
        assert!(command.contains("--nice-name=mahiroute-helper"));
    }

    #[test]
    fn the_command_never_matches_its_own_process_by_name() {
        // `kill $(pidof …)` is exact-match on the process name, unlike `pkill -f`, so the shell
        // that runs this command is safe from it.
        let command = launch_command(APK, "0123456789abcdef", PACKAGE).expect("command");
        assert!(!command.contains("pkill"));
    }

    #[test]
    fn unsafe_inputs_are_refused() {
        for apk in [
            "/data/app/x'; reboot; '/base.apk",
            "relative/base.apk",
            "/data/app/x.apk; id",
            "/data/app/x.so",
            "/d a/b.apk",
        ] {
            assert!(
                launch_command(apk, "0123456789abcdef", PACKAGE).is_err(),
                "{apk:?} must be refused"
            );
        }
        for token in ["short", "zzzzzzzzzzzzzzzzzz", "0123456789abcdef; id"] {
            assert!(
                launch_command(APK, token, PACKAGE).is_err(),
                "{token:?} must be refused"
            );
        }
    }

    #[test]
    fn ready_line_is_found_among_noise() {
        let output = "some warning\r\nMAHIROUTE_READY 40123\r\n";
        assert_eq!(parse_ready(output).expect("port"), 40123);
        assert!(parse_ready("").is_err());
        assert!(parse_ready("MAHIROUTE_READY nope").is_err());
        assert!(parse_ready("MAHIROUTE_READY 99999").is_err());
    }

    #[test]
    fn tokens_are_long_hex_and_unique() {
        let first = generate_token().expect("token");
        let second = generate_token().expect("token");
        assert_eq!(first.len(), 32);
        assert!(first.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(first, second);
    }
}
