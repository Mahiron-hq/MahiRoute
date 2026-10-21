# Security policy

## Supported versions

Security fixes go into the latest release only.

| Version | Supported |
|---|---|
| 1.0.x | yes |

## Reporting a vulnerability

Please **do not open a public issue** for a security problem. Use GitHub's private reporting:
**Security → Advisories → Report a vulnerability** in this repository. Include what you found, how
to reproduce it, the phone model and Android version, and the MahiRoute version. This is a
one-person project, so replies are best effort, but every report is read.

## What is in scope

MahiRoute starts a helper that runs with the Android **shell** user. Whatever can talk to that
helper can change the phone's audio routing, so these areas matter most:

- the helper's local socket and its token check (`app/src/main/java/.../helper/`);
- the handling of the ADB key, the pairing code and the start script;
- parsing of helper messages and of the settings file (malformed input must never crash or corrupt the engine);
- anything that lets another app on the phone start, stop or steer the helper without the token.

Out of scope: attacks that need root, a debugging connection you already authorized, or physical access to an unlocked phone.

## How the helper is protected

- It listens on `127.0.0.1` only, on a port chosen at start, and every connection must first present a random token.
- A connection that does not authenticate within five seconds is closed, and only an authenticated connection can replace the current one, so a stray connect cannot evict the app.
- The token lives in the app's private storage and in the start script in the app's own folder on shared storage (which other apps cannot normally read on Android 11 and newer). A helper started through wireless debugging gets a fresh token every time; the one started from a computer keeps a stored token.
- The helper records every change it makes in its state file and undoes it when it stops or restarts.
- App data is excluded from cloud backup and device transfer.

See [PRIVACY.md](PRIVACY.md) for what is stored where.
