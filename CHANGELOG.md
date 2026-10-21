# Changelog

All notable changes to MahiRoute are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [1.0.1] - 2026-10-21

### Fixed

- Bluetooth headphones disappeared from the app, and could not be chosen, when a wired headset was plugged in while they were connected: Android deactivates the active Bluetooth device in that case and stops listing it. They now stay in the list and choosing them activates the connection again.
- The continuous integration build failed because the Gradle wrapper script was not marked executable.

### Changed

- The diagnostics note for a device Android does not currently offer is worded neutrally.
- The README no longer carries build instructions and lists Bluetooth, USB DAC and wired-headphone checks in the tested table.

## [1.0.0] - 2026-10-21

First public release.

### Added

- System-wide choice of the microphone and of the sound output for every app, with live device lists (built-in, wired, USB, Bluetooth).
- A privileged helper that applies the choice through Android's audio policy: a preferred device per capture preset and product strategy, plus hiding of competing devices on firmware that ignores preferences (measured per firmware and remembered).
- Verification with real test streams before the interface reports "Apps use the chosen devices".
- Handling of unplugged devices: the field shows the system default, the warning names the device, the choice is remembered and applied again when the device returns; the warning can be closed and stays closed across restarts until the device returns or another one is chosen. USB devices are told apart by their product name.
- Helper start through wireless debugging with in-app pairing (code entered in a notification), automatic restart after reboots and failures, and a clear note that the Wi-Fi network does not need internet access. The start waits for Android's "allow on this network?" prompt and continues by itself once it is answered, and it waits for the user to switch wireless debugging on when the app is not yet allowed to.
- Helper start from a computer for phones without Wi-Fi: a script on the phone, a Windows launcher (`MahiRoute-start.bat`, English and Russian) that the app can save to Downloads, and automatic pickup of the helper on a fixed local port.
- "Start after a restart" switch, Quick Settings tile, status notification with an off action, and alerts for disconnected and restored devices.
- Diagnostics screen: helper state, Android version, measured firmware behavior, connected devices including hidden ones, and active audio streams.
- English and Russian interface.

### Security

- The helper listens on loopback only, requires a random token, drops connections that do not authenticate within five seconds, and lets only an authenticated connection replace the current one.
- App data is excluded from cloud backup and device transfer.
