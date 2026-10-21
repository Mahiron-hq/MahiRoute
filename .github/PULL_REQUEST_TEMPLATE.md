## What changed and why

## How it was tested

- Phone and Android version (routing cannot be judged from unit tests alone):
- [ ] `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-features`
- [ ] `./gradlew :app:testDebugUnitTest :app:lintDebug :app:assembleDebug`
- [ ] If the state sent to the UI changed: `ui_state_contract.json` regenerated and `UiModel` updated
- [ ] New files carry the `SPDX-License-Identifier` and copyright header
- [ ] No keys, tokens or personal data
