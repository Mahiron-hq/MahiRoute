# Architecture

MahiRoute lets the user choose the microphone and the sound output for every app on an Android
phone. Android only lets the system (or a process with the `MODIFY_AUDIO_ROUTING` permission) do
that, so the app runs a small helper with the `shell` user's rights and drives it from a Rust engine.

```
 MahiRoute app (Kotlin, Jetpack Compose)
   │ JNI                                    screens, notifications, tile, service, bootstrap
 Engine (Rust, libmahiroute_jni.so)
   │ loopback TCP + token, JSON lines       planner, state machine, verification, ADB client
 Helper (Java, uid shell, app_process)
   │ AudioManager / AudioSystem (hidden APIs)
 Android audio policy ──► chosen microphone and output
```

## Repository layout

| Path | What it is |
|---|---|
| `rust/crates/core` | Platform-independent logic: devices and their ranking, the routing planner, the helper wire protocol, persistent settings. No Android, no sockets. |
| `rust/crates/engine` | The pure state machine `Core` (commands, helper messages and timers in, effects out) and the threaded `Engine` that runs it with a TCP connector, heartbeat and reconnect backoff. |
| `rust/crates/adb` | An ADB client for the phone's own wireless debugging: key and certificate, SPAKE2 pairing, TLS 1.3 session, the `shell` service, and the command that starts the helper. |
| `rust/crates/jni` | The JNI bridge: one engine per process, commands as typed calls, state as a JSON document. |
| `app/src/main/java/.../helper` | The helper (plain Java, started with `app_process`). |
| `app/src/main/java/.../core`, `bootstrap`, `service`, `ui`, … | The Android app. |
| `app/src/main/assets/MahiRoute-start.bat` | The Windows launcher for starting the helper over USB. |

## The routing planner

`plan_route(selection, devices, capabilities)` is a pure function that returns a declarative
`DesiredRouting`: a preferred input, a preferred output and a list of devices to **mask**.

- Every capture preset and product strategy gets the chosen device as its preferred device.
- Some firmware stores the preference but does not obey it (measured on MIUI). The engine learns this
  per firmware fingerprint (`Capabilities`: `Unknown`, `Honored`, `Ignored`). When a preference is
  ignored, the planner also masks every device that the platform ranks above the choice (a wired
  headset microphone outranks the built-in one) and the sibling built-in microphone, because some call
  routes swap the chosen microphone for its neighbour.
- Masking means `AudioSystem.setDeviceConnectionState(attributes, UNAVAILABLE, "")`: the audio policy
  stops seeing the device until the helper announces it again.
- A selected device that is not connected is reported as `missing`; the plan then leaves that
  direction to the system.

Applying a plan twice is a no-op, so the engine can reconcile freely whenever devices change.

## Verification

After an `Apply` the engine asks the helper to open short test streams: recording with the sources
`MIC` and `VOICE_COMMUNICATION`, and a policy query for the usages `MEDIA` and `VOICE_COMMUNICATION`.
The helper reports the device each one was routed to. If a preference is not obeyed the engine
escalates that direction to `Ignored`, persists the finding and re-plans with masks; if routing is
still wrong it reports `Failed` instead of looping. The interface shows "Apps use the chosen devices"
only after verification succeeded.

## The helper

`HelperMain` runs as the shell user (it exits otherwise) and listens on `127.0.0.1`.

- **Protocol:** newline-delimited JSON. A connection must open with `hello` carrying the token; then
  `list_devices`, `apply`, `clear_all`, `diagnostics`, `probe` and `shutdown`. The helper pushes
  `devices`, `mode` and `streams` events.
- **Authentication:** a connection that does not authenticate within five seconds is closed, and only an
  authenticated connection can replace the current one.
- **Undo:** every mask and preference it sets is written to `/data/local/tmp/mahiroute-state.json`
  first. A helper that starts later undoes whatever its predecessor left behind, and the state is
  reconciled with what the audio policy actually shows (the policy forgets masks on reboot).
- **Mode changes:** the framework clears the communication strategy's preference when the audio mode
  changes, so the helper asserts it again a few times after each mode event.
- **Probes** use an `AudioRecord` attributed to `com.android.shell` (needed since Android 14) and the
  policy query `getAudioDevicesForAttributes` for playback, which opens no stream.

## Starting the helper

Without root the only way to get shell rights is Android's debugging service.

1. **Wireless debugging (default).** The app discovers `_adb-tls-connect._tcp` over NSD, pairs once
   with the six-digit code (the notification's inline reply keeps the Settings dialog open), connects
   with TLS 1.3 using its own key, and runs the start command through the `shell` service. The first
   successful start also grants the app `WRITE_SECURE_SETTINGS`, so later starts switch wireless
   debugging on by themselves.
   - Android asks "Allow wireless debugging on this network?" the first time on a network and puts the
     setting back to off until the user answers. `StartLoop` therefore waits up to two minutes for the
     service instead of one short attempt, skips ports that already refused a connection, and
     `DebuggingKeeper` writes the setting again once the prompt is answered. Without that the first
     attempt ended without a helper and a second tap was needed.
   - Without the permission only the user can switch debugging on, so the start waits for that and
     continues by itself.
   - After a reboot or a failure `AutoStarter` repeats the start every 30 seconds while Wi-Fi is
     available.
2. **From a computer.** The app writes `start-helper.sh` into its folder on shared storage; the script
   starts the helper on the fixed port 41999 with a stored token. `MahiRoute-start.bat` runs it
   through `adb`, and the app notices the listening port and connects by itself.

Wireless debugging needs an associated Wi-Fi network but no internet access. See
[FINDINGS.md](FINDINGS.md) for why neither a computer nor Wi-Fi can be avoided without root.

## The app

- `EngineHost` owns the engine and publishes `UiModel`, parsed from the engine's JSON state.
- `RoutingService` is a foreground service that keeps the process alive while a device is chosen and
  raises the alerts about disconnected and restored devices.
- `StartupPolicy` stores the "start after a restart" switch and enables or disables `BootReceiver`
  accordingly, so a disabled setting means Android does not start the app at boot at all.
- Notifications, the Quick Settings tile and the diagnostics screen are thin views over `UiModel`.

## Testing

- `core`: unit tests and `proptest` round-trips for devices, the planner (including masks and
  siblings), the protocol and the settings store.
- `engine`: scenario tests drive `Core` against a model of the helper (honoring and ignoring firmware,
  unplug and replug, closed warnings across restarts); a TCP test runs the real driver.
- `adb`: a stand-in for `adbd` exercises pairing, the TLS session and the helper start command.
- The JSON the engine sends to the interface is pinned by `app/src/test/resources/ui_state_contract.json`;
  a Rust test regenerates and compares it and a Kotlin test parses it.
- Kotlin unit tests cover the start loop's timing on a virtual clock and the debugging keeper's policy.
- Behavior on real phones is recorded in [FINDINGS.md](FINDINGS.md).

## Security notes

The helper can change audio routing, nothing else; it is reachable only on loopback and only with the
token. Details and what to report are in [SECURITY.md](../SECURITY.md); what is stored where is in
[PRIVACY.md](../PRIVACY.md).
