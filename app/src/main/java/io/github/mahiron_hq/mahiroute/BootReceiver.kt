// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

package io.github.mahiron_hq.mahiroute

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import io.github.mahiron_hq.mahiroute.core.EngineHost
import io.github.mahiron_hq.mahiroute.startup.StartupPolicy

/** Brings the engine back after a reboot; the application object does the rest. */
class BootReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action != Intent.ACTION_BOOT_COMPLETED) return
        if (StartupPolicy.isStartOnBoot(context)) EngineHost.start(context)
    }
}
