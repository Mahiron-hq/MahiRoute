# Contributing to MahiRoute

Thanks for helping. Bug reports with real device data are the most valuable contribution: the
behavior of Android's audio policy differs between firmware, and this project can only be as good
as the phones it has been tried on.

## Reporting a bug

Open an issue with the **Bug report** form. Please include the phone model, the Android version and
ROM, how the helper was started, and a copy of what the **Diagnostics** screen shows (the "How this
firmware behaves" lines and the connected devices). Security problems go through
[SECURITY.md](SECURITY.md), not a public issue.

## Building

You need JDK 17, the Android SDK (platform 36, build-tools 36.1.0, NDK 29), Rust 1.95 with the
`aarch64-linux-android` target, `cargo-ndk` 4.1.2 and CMake. The exact commands are in the
[README](README.md#build).

## Before you open a pull request

Run, and make sure they all pass:

```
cd rust
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo build --all-targets        # no warnings
cargo test --all-features

cd ..
./gradlew :app:testDebugUnitTest :app:lintDebug :app:assembleDebug
```

Guidelines:

- **Rust:** `unsafe` is denied by default; the one allowed module (BoringSSL FFI for SPAKE2) carries a `SAFETY` comment. Public items are documented, errors use `thiserror`, and no `unwrap` on anything that crosses a process boundary. Logic that has a round-trip property gets a `proptest`.
- **Kotlin and Java:** official Kotlin style, no new warnings (warnings are errors in the app module).
- **Comments** say why, not what. Keep them short.
- The state that Rust sends to the Kotlin UI is pinned by `app/src/test/resources/ui_state_contract.json`. If you change it on purpose, regenerate it with `UPDATE_GOLDEN=1 cargo test -p mahiroute-engine --test ui_contract` and update `UiModel` in the same commit.
- Add tests with the change: unit tests for logic, scenario tests in `rust/crates/engine/tests` for behavior of the engine.
- Say which phone and Android version you tested on. Routing behavior cannot be judged from unit tests alone.
- Never commit signing keys, tokens or personal data.

## Commits and pull requests

Write a short imperative subject that says what changed and why it matters. Keep one topic per pull
request. Fill in the pull request template.

## License

By contributing you agree that your contribution is licensed under the
[GNU Affero General Public License, version 3 or later](LICENSE), like the rest of the project, and
that it carries the same two-line `SPDX-License-Identifier` and copyright header as the files around it.
