// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

package io.github.mahiron_hq.mahiroute.startup

import android.content.ComponentName
import android.content.Context
import android.content.pm.PackageManager
import androidx.core.content.edit
import io.github.mahiron_hq.mahiroute.BootReceiver
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * Whether the app starts by itself after the phone restarts. The preference lives in plain shared
 * preferences because the boot receiver needs it before the engine is up.
 */
object StartupPolicy {
    private const val PREFS = "startup"
    private const val KEY_START_ON_BOOT = "start_on_boot"

    private val mutableStartOnBoot = MutableStateFlow(true)
    val startOnBoot: StateFlow<Boolean> = mutableStartOnBoot.asStateFlow()

    /** Reads the stored choice and makes the boot receiver match it; call once per process. */
    fun load(context: Context) {
        val stored = isStartOnBoot(context)
        mutableStartOnBoot.value = stored
        applyToReceiver(context, stored)
    }

    fun isStartOnBoot(context: Context): Boolean =
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).getBoolean(KEY_START_ON_BOOT, true)

    fun setStartOnBoot(context: Context, enabled: Boolean) {
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit { putBoolean(KEY_START_ON_BOOT, enabled) }
        mutableStartOnBoot.value = enabled
        applyToReceiver(context, enabled)
    }

    /** A disabled receiver gets no boot broadcast, so Android does not even start the app process. */
    private fun applyToReceiver(context: Context, enabled: Boolean) {
        val state =
            if (enabled) PackageManager.COMPONENT_ENABLED_STATE_DEFAULT else PackageManager.COMPONENT_ENABLED_STATE_DISABLED
        context.packageManager.setComponentEnabledSetting(
            ComponentName(context, BootReceiver::class.java),
            state,
            PackageManager.DONT_KILL_APP,
        )
    }
}
