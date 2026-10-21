# Findings

Measurements made with MahiRoute and with small throw-away probe apps on real phones. They drove the
design: preferences first, device masking as a fallback chosen by a per-firmware probe.

## Test phones

| Phone | Android | Notes |
|---|---|---|
| Redmi Note 10S (M2101K7BNY) | 13, MIUI 14 | 3.5 mm headphones are detected as a headset **with** a microphone, which is the problem this project solves |
| Infinix X6855 | 16 | No headphone jack; the phone shares its connection, so it cannot use wireless debugging |

## Audio routing

- **Redmi / MIUI 14.** `setPreferredDeviceForCapturePreset` is stored by the policy but ignored when the
  input is chosen. Hiding the wired headset microphone with
  `AudioSystem.setDeviceConnectionState(attributes, 0, "")` works; announcing it again (state 1) brings
  it back with a new port id. The preferred output per product strategy works for media. In call mode
  with the speaker as output the platform swaps the microphone for the "back" one, so the sibling
  built-in microphone has to be hidden as well.
- **Infinix / Android 16.** Capture-preset and strategy preferences are honored, including in call mode
  (`MODE_IN_COMMUNICATION`). Its default microphone is the "back" one, not "bottom".
- On Android 13 `AudioProductStrategy.getName()` does not exist; `toString()` is used.
- The framework clears the communication strategy's preferred device on a mode change on the Redmi, so
  the helper asserts it again after mode events.
- Shell-side probe recordings can report a level of 0 (the shell is muted by app-ops); the routed device
  is what counts, not the level.
- After a reboot the audio policy no longer hides what the helper hid before, while the helper's state
  file still lists those masks. The helper therefore checks recorded masks against the devices the
  policy shows instead of trusting the file.

## Why a computer or Wi-Fi is still needed

Question: can the phone start the helper without a Wi-Fi network and without a PC? Without root, no.
A process with shell rights can only be started through `adbd`, and `adbd` is reachable over USB
(from a computer) or through wireless debugging. Everything else was tried on the Redmi:

- **Wireless debugging needs a Wi-Fi connection.** With Wi-Fi off, `adb_wifi_enabled` went to 0 at once
  and the port of `adbd` disappeared. Writing `settings put global adb_wifi_enabled 1` (the app has
  `WRITE_SECURE_SETTINGS`) did not help. The system log says `Network disconnected. Disabling adbwifi.`
  and `Not connected to any wireless network. Not enabling adbwifi.` The phone's own hotspot does not
  count as a connection. The network needs no internet access.
- **`adb tcpip` does not survive a reboot.** After `adb tcpip 5555` the port worked; after a reboot it
  was gone (`persist.adb.tcp.port` stayed empty). The shell user cannot set `persist.adb.tcp.port`
  (`Failed to set property`). Restarting `adbd` also took the helper down with it (observed).
- **The permission cannot be granted directly.** `pm grant … MODIFY_AUDIO_ROUTING` fails: it is a
  signature permission.
- **Public APIs do not reach other apps.** `AudioManager.setCommunicationDevice` only affects the
  calling app's own calls.

What this means in practice: the helper lives until the phone reboots and survives app restarts and
updates. After a reboot it comes back by itself when any Wi-Fi network is available (no internet
needed), or by running `MahiRoute-start.bat` over USB.

## Wireless debugging prompt

On a network that Android does not trust yet, switching wireless debugging on shows "Allow wireless
debugging on this network?" and resets the setting to off. The user's "OK" starts the server for
about 75 ms and shuts it down again, because the setting is still off; only a later write of the
setting makes it stick. The start therefore waits for the user, re-writes the setting once the prompt
is closed, and ignores ports that refuse connections (the old announcement of the server that just
shut down stays visible for a while). This was reproduced on the Redmi by revoking the debugging
authorizations (which also forgets trusted networks) and answering the prompt after more than 45 seconds.

## Firmware that is not covered

Only the two phones above were tested. Bluetooth devices and USB DACs were not tested; they use the
same mechanisms. Bug reports with the diagnostics screen are the best way to
extend this table; see [CONTRIBUTING.md](../CONTRIBUTING.md).
