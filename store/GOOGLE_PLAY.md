# Publishing MahiRoute on Google Play

Checklist and ready texts for the project owner. Google Play policies change often, so check every
point against the current Play Console help before submitting; this page does not replace it.
For RuStore see [RUSTORE.md](RUSTORE.md).

## 1. Before the first upload

- A Play Console developer account and an app signing setup. Use Play App Signing; keep the upload
  key private and never commit it. A release already signed with a different key on GitHub cannot be
  updated from Play (and the other way round), so decide which key is the long-term one first.
- Build an Android App Bundle (`./gradlew :app:bundleRelease`); Play does not accept new APK uploads.
- `targetSdk` is 36, which satisfies the current target API requirement. Re-check each year.
- Privacy policy URL: <https://github.com/Mahiron-hq/MahiRoute/blob/main/PRIVACY.md>. The same text is shown
  in the app on the first start.
- Content rating questionnaire: no user-generated content, no ads, no purchases. Target audience: adults.
- Data safety form: data collected: none; data shared: none; data encrypted in transit: not applicable
  (nothing is transmitted); users can request deletion: not applicable (nothing is stored off the phone).

## 2. Listing texts

**Title (30):** MahiRoute: mic and sound output

**Short description (80):** Choose the microphone and sound output that every app on your phone uses.

**Full description:**

> Plugged in headphones without a microphone and Android silently switched recording to a microphone that
> does not exist? Voice messages and calls get quiet. MahiRoute lets you choose.
>
> - Pick the built-in, USB or Bluetooth microphone and any speaker or headphones. Every app (Telegram,
>   Discord, the voice recorder and others) follows your choice.
> - The result is checked with real test streams instead of being taken on trust.
> - If the chosen device is unplugged, MahiRoute remembers the choice and applies it again when the
>   device returns. Bluetooth headphones stay selectable after a wired headset is plugged in.
> - Quick Settings tile, diagnostics, start after a restart (can be switched off), language and theme
>   settings.
> - No ads, no analytics, no accounts. Nothing leaves your phone. Free and open source (AGPL-3.0).
>
> How it works: only the system may change the audio route on Android, so after each restart MahiRoute
> starts a small helper through Android's own Wireless debugging. A Wi-Fi network is required, but it
> does not need internet access. Phones without Wi-Fi can start the helper from a computer over USB.
> Step-by-step help is in the app and on the project page.

**What is new (1.0.2):** Privacy notice in the app, settings (language, theme, device alerts,
diagnostics report, reset), an about screen, and a fix for Bluetooth headphones vanishing from the list
when a wired headset is plugged in.

Screenshots: `docs/screenshots/*.png` (English) and `docs/screenshots/*-ru.png` (Russian), phone size.

## 3. Declarations and review notes

Reviewer notes (App access): the app needs a phone with Android 12 or newer on Wi-Fi (no internet
needed). Open the app, accept the notice, pick devices, tap Start the helper, turn on Wireless
debugging, pair with the code from the notification the first time. The screen then reports that apps
use the chosen devices.

| Permission | Declaration |
|---|---|
| `WRITE_SECURE_SETTINGS` | Not grantable at install; granted by the user's own `pm grant` through the helper to switch Wireless debugging on. Explain it as a user-initiated setup step |
| `QUERY_ALL_PACKAGES` | Needs the Play declaration form. Used only to name apps that play or record sound on the diagnostics screen. Not a core function, so removal is the safe option (see risks) |
| `FOREGROUND_SERVICE_SPECIAL_USE` | Needs a declaration with a short video: keeps the user's chosen audio routing alive in the background |
| `REQUEST_IGNORE_BATTERY_OPTIMIZATIONS` | Allowed only when background operation is a core function; routing must stay in place, so declare it, and the user may refuse |
| `RECEIVE_BOOT_COMPLETED`, `POST_NOTIFICATIONS`, `BLUETOOTH_CONNECT` | Core functions, switchable or runtime-granted |
| `INTERNET`, network and Wi-Fi state, multicast | Local sockets and discovery of the Wireless debugging service only |

## 4. Known risks

- **Rejection for `QUERY_ALL_PACKAGES` or the shell-helper approach.** Play is strict about both. If
  asked, remove `QUERY_ALL_PACKAGES` (the diagnostics screen then shows streams without app names) and
  keep the explanation that the helper only runs what Android's own debugging permits, with the
  user's consent, and sends nothing anywhere.
- **Account requirements.** New personal developer accounts may have to run a closed test with a
  minimum number of testers before production access; plan time for it.
- **One commit, one key.** Publish the same commit that is released on GitHub, signed with the key
  that will be used for every later update.
