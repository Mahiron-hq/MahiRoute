// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

package io.github.mahiron_hq.mahiroute.core

import android.content.Context
import android.util.Log
import java.io.File
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/** Process-wide owner of the Rust engine and the latest state it published. */
object EngineHost {
    private const val TAG = "MahiRoute"

    private val mutableState = MutableStateFlow(UiModel.EMPTY)
    val state: StateFlow<UiModel> = mutableState.asStateFlow()

    private var started = false

    @Synchronized
    fun start(context: Context) {
        if (started) return
        started = true
        val settings = File(context.filesDir, "settings.json")
        Native.engineStart(settings.absolutePath) { json ->
            try {
                mutableState.value = UiModel.parse(json)
            } catch (error: org.json.JSONException) {
                Log.e(TAG, "unreadable engine state", error)
            }
        }
    }

    fun setEnabled(enabled: Boolean) = Native.setEnabled(enabled)

    /** Chooses [device], or the system default when it is null. */
    fun choose(direction: Direction, device: DeviceKey?) {
        if (!Native.choose(direction == Direction.Input, device?.toJson())) {
            Log.w(TAG, "engine rejected the choice $device for $direction")
        }
    }

    /** Hides the missing-device warning until the device returns or another one is chosen. */
    fun dismissMissing(direction: Direction) = Native.dismissMissing(direction == Direction.Input)

    fun connect(port: Int, token: String): Boolean = Native.connect(port, token)

    fun disconnect() = Native.disconnect()

    fun recalibrate() = Native.recalibrate()
}
