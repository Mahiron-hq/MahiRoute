# Privacy

MahiRoute collects nothing. It has no accounts, no ads, no analytics, no crash reporting and no
server. It never connects to the internet and sends no data to the author or to anyone else.

## What the app talks to

- **The helper on the same phone**, over a loopback socket (`127.0.0.1`) protected by a random token.
- **Android's own wireless-debugging service on the same phone**, over TLS, to start the helper and to pair once. The `INTERNET` permission exists only for these local sockets.
- **Nothing else.** Wi-Fi and network state permissions are used to notice that Wi-Fi is available and to find the wireless-debugging service on the local network (mDNS).

## Permissions and why

| Permission | Why |
|---|---|
| `INTERNET`, `ACCESS_NETWORK_STATE`, `ACCESS_WIFI_STATE`, `CHANGE_WIFI_MULTICAST_STATE` | Local sockets, Wi-Fi detection, mDNS discovery of wireless debugging |
| `WRITE_SECURE_SETTINGS` | Switch wireless debugging on by itself. The helper start grants it with `pm grant`; it is not requested at install time |
| `POST_NOTIFICATIONS` | Status, device alerts, the pairing-code prompt |
| `BLUETOOTH_CONNECT` | List and select Bluetooth audio devices |
| `FOREGROUND_SERVICE`, `FOREGROUND_SERVICE_SPECIAL_USE` | Keep routing alive while a device is chosen |
| `RECEIVE_BOOT_COMPLETED` | Start again after a restart (can be switched off in the app) |
| `REQUEST_IGNORE_BATTERY_OPTIMIZATIONS` | Keep Android from stopping the app in the background |
| `QUERY_ALL_PACKAGES` | Show the names of apps that play or record sound on the diagnostics screen |

## Audio

To check routing the helper opens very short test streams. The microphone test records about
0.3 seconds at 16 kHz, measures the signal level and the device it was routed to, and discards the
samples in memory. Audio is never stored, played back or sent anywhere.

## Notice and acceptance in the app

The first time the app opens, and again whenever this notice changes in substance, it shows the
notice and asks you to accept it. Until you accept, no permission is requested and nothing else in
the interface is available. You can read the notice later under Settings, and the app shows the
accepted version only on the phone.

## Diagnostics report

Settings can copy or share a plain-text report for bug reports. It holds the app and Android
versions, the phone model, the helper state, the kind of each audio device and the engine warnings.
Hardware addresses are replaced, and device names and the names of apps are left out. The report
leaves the phone only when you tap Share and pick where to send it.

## Data stored on the phone

- In the app's private storage: `settings.json` (your device choices, measured firmware behavior,
  closed warnings), the interface settings (language, theme, device alerts), the accepted version of
  this notice, the ADB key used for pairing, and the helper's address and token.
- In the app's folder on shared storage (`Android/data/io.github.mahiron_hq.mahiroute/files/`):
  `start-helper.sh`, the script a computer runs through `adb`; it contains the helper token.
- In `/data/local/tmp/` (reachable only through the shell user): the helper's state file and log,
  which hold device names and the routing state.
- Backups are disabled: app data is excluded from cloud backup and device transfer.

Uninstalling the app removes the first two groups; the files in `/data/local/tmp/` are removed when the
phone restarts or with `adb shell rm`.

## Third parties

No third-party service or SDK is used. Third-party libraries compiled into the app are listed in
[NOTICE](NOTICE).

## Questions

Open an issue at <https://github.com/Mahiron-hq/MahiRoute/issues>.
