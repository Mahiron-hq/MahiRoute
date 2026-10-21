// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

package io.github.mahiron_hq.mahiroute.core

/** Receives every engine state as a JSON document; called on the engine thread. */
fun interface StateListener {
    fun onState(json: String)
}

/** Entry points implemented by the Rust engine (`libmahiroute_jni.so`). */
internal object Native {
    init {
        System.loadLibrary("mahiroute_jni")
    }

    @JvmStatic
    external fun engineStart(settingsPath: String, listener: StateListener)

    @JvmStatic
    external fun engineStop()

    @JvmStatic
    external fun setEnabled(enabled: Boolean)

    /** Picks a device (or the system default for a null key); false when the key is invalid. */
    @JvmStatic
    external fun choose(input: Boolean, deviceKeyJson: String?): Boolean

    /** Closes the warning about the chosen device of one direction being disconnected. */
    @JvmStatic
    external fun dismissMissing(input: Boolean)

    @JvmStatic
    external fun connect(port: Int, token: String): Boolean

    @JvmStatic
    external fun disconnect()

    @JvmStatic
    external fun recalibrate()

    /** Pairs with wireless debugging; returns JSON `{"ok":true}` or `{"error":…,"kind":…}`. */
    @JvmStatic
    external fun adbPair(port: Int, code: String, keyPath: String, peerName: String): String

    /** Starts the helper through wireless debugging; returns JSON with `port` and `token`, or an error. */
    @JvmStatic
    external fun adbStartHelper(port: Int, apkPath: String, keyPath: String, packageName: String): String
}
