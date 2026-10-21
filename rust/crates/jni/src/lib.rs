// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

//! JNI bridge between the Android app and the Rust engine.
//!
//! The app owns exactly one engine per process, so the bridge keeps it in a process-wide slot
//! instead of handing raw pointers to Kotlin. Commands are typed JNI calls; state flows back as
//! a JSON document through `StateListener.onState`.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

mod bridge;

use jni::errors::ThrowRuntimeExAndDefault;
use jni::objects::{JClass, JObject, JString};
use jni::refs::Global;
use jni::sys::jboolean;
use jni::{EnvUnowned, JValue, JavaVM, jni_mangle, jni_sig, jni_str};
use mahiroute_core::{Choice, DeviceKey, Direction};
use mahiroute_engine::{Command, Engine, HelperTarget, Listener, TcpConnector, UiState};

static ENGINE: Mutex<Option<Engine>> = Mutex::new(None);

fn engine_slot() -> MutexGuard<'static, Option<Engine>> {
    // A poisoned lock only means a panic happened elsewhere; the slot itself stays valid.
    ENGINE.lock().unwrap_or_else(PoisonError::into_inner)
}

fn send(command: Command) {
    if let Some(engine) = engine_slot().as_ref() {
        engine.command(command);
    }
}

/// Forwards every state change to the Kotlin `StateListener`.
struct JavaListener {
    target: Global<JObject<'static>>,
}

impl Listener for JavaListener {
    fn state_changed(&self, state: &UiState) {
        let Ok(json) = serde_json::to_string(state) else {
            return;
        };
        let Ok(vm) = JavaVM::singleton() else {
            return;
        };
        // A listener that throws or a VM that is shutting down must not take the engine down.
        let _ = vm.attach_current_thread(|env| -> jni::errors::Result<()> {
            let text = JString::from_str(env, &json)?;
            env.call_method(
                &self.target,
                jni_str!("onState"),
                jni_sig!((json: java.lang.String) -> void),
                &[JValue::Object(&text)],
            )?;
            Ok(())
        });
    }
}

/// `Native.engineStart(settingsPath, listener)`: starts the engine, replacing a running one.
#[jni_mangle("io.github.mahiron_hq.mahiroute.core.Native")]
pub fn engine_start<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    settings_path: JString<'local>,
    listener: JObject<'local>,
) {
    unowned_env
        .with_env(|env| -> jni::errors::Result<()> {
            let path = PathBuf::from(settings_path.to_string());
            let target = env.new_global_ref(&listener)?;
            let engine = Engine::start(
                path,
                Arc::new(TcpConnector),
                Arc::new(JavaListener { target }),
            );
            // Dropping a previous engine stops its thread before the new one takes over.
            *engine_slot() = Some(engine);
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>();
}

/// `Native.engineStop()`: stops the engine and releases the listener.
#[jni_mangle("io.github.mahiron_hq.mahiroute.core.Native")]
pub fn engine_stop<'local>(mut unowned_env: EnvUnowned<'local>, _class: JClass<'local>) {
    unowned_env
        .with_env(|_env| -> jni::errors::Result<()> {
            *engine_slot() = None;
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>();
}

/// `Native.setEnabled(enabled)`: master switch.
#[jni_mangle("io.github.mahiron_hq.mahiroute.core.Native")]
pub fn set_enabled<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    enabled: jboolean,
) {
    unowned_env
        .with_env(|_env| -> jni::errors::Result<()> {
            send(Command::SetEnabled(enabled));
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>();
}

/// `Native.choose(input, deviceKeyJson)`: picks a device, or the system default when the key is null.
///
/// Returns false when the key is not valid JSON for a device of the requested direction.
#[must_use]
#[jni_mangle("io.github.mahiron_hq.mahiroute.core.Native")]
pub fn choose<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    input: jboolean,
    device_key_json: JString<'local>,
) -> jboolean {
    unowned_env
        .with_env(|_env| -> jni::errors::Result<jboolean> {
            let direction = if input {
                Direction::Input
            } else {
                Direction::Output
            };
            let choice = if device_key_json.is_null() {
                Choice::SystemDefault
            } else {
                match serde_json::from_str::<DeviceKey>(&device_key_json.to_string()) {
                    Ok(key) if key.direction == direction => Choice::Device(key),
                    _ => return Ok(false),
                }
            };
            send(Command::Choose { direction, choice });
            Ok(true)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `Native.dismissMissing(input)`: closes the warning about a disconnected device.
#[jni_mangle("io.github.mahiron_hq.mahiroute.core.Native")]
pub fn dismiss_missing<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    input: jboolean,
) {
    unowned_env
        .with_env(|_env| -> jni::errors::Result<()> {
            send(Command::DismissMissing(if input {
                Direction::Input
            } else {
                Direction::Output
            }));
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>();
}

/// `Native.connect(port, token)`: points the engine at a running helper.
#[must_use]
#[jni_mangle("io.github.mahiron_hq.mahiroute.core.Native")]
pub fn connect<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    port: jni::sys::jint,
    token: JString<'local>,
) -> jboolean {
    unowned_env
        .with_env(|_env| -> jni::errors::Result<jboolean> {
            let Ok(port) = u16::try_from(port) else {
                return Ok(false);
            };
            send(Command::Connect(HelperTarget {
                port,
                token: token.to_string(),
            }));
            Ok(true)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `Native.disconnect()`: forgets the helper endpoint.
#[jni_mangle("io.github.mahiron_hq.mahiroute.core.Native")]
pub fn disconnect<'local>(mut unowned_env: EnvUnowned<'local>, _class: JClass<'local>) {
    unowned_env
        .with_env(|_env| -> jni::errors::Result<()> {
            send(Command::Disconnect);
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>();
}

/// `Native.recalibrate()`: forgets measured firmware behavior and measures again.
#[jni_mangle("io.github.mahiron_hq.mahiroute.core.Native")]
pub fn recalibrate<'local>(mut unowned_env: EnvUnowned<'local>, _class: JClass<'local>) {
    unowned_env
        .with_env(|_env| -> jni::errors::Result<()> {
            send(Command::Recalibrate);
            Ok(())
        })
        .resolve::<ThrowRuntimeExAndDefault>();
}

/// `Native.adbPair(port, code, keyPath, peerName)`: pairs with wireless debugging on this phone.
///
/// Returns JSON: `{"ok":true}` or `{"error":…,"kind":…}`. Blocks for up to a few seconds, so call
/// it off the main thread.
#[must_use]
#[jni_mangle("io.github.mahiron_hq.mahiroute.core.Native")]
pub fn adb_pair<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    port: jni::sys::jint,
    code: JString<'local>,
    key_path: JString<'local>,
    peer_name: JString<'local>,
) -> JString<'local> {
    unowned_env
        .with_env(|env| -> jni::errors::Result<JString<'local>> {
            JString::from_str(
                env,
                bridge::pair(
                    port,
                    &code.to_string(),
                    &key_path.to_string(),
                    &peer_name.to_string(),
                ),
            )
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// `Native.adbStartHelper(port, apkPath, keyPath)`: starts the helper through wireless debugging.
///
/// Returns JSON: `{"port":…,"token":"…"}` on success, `{"error":"…"}` otherwise. Blocks for up to
/// half a minute, so call it off the main thread.
#[must_use]
#[jni_mangle("io.github.mahiron_hq.mahiroute.core.Native")]
pub fn adb_start_helper<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    port: jni::sys::jint,
    apk_path: JString<'local>,
    key_path: JString<'local>,
    package: JString<'local>,
) -> JString<'local> {
    unowned_env
        .with_env(|env| -> jni::errors::Result<JString<'local>> {
            JString::from_str(
                env,
                bridge::start_helper(
                    port,
                    &apk_path.to_string(),
                    &key_path.to_string(),
                    &package.to_string(),
                ),
            )
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}
