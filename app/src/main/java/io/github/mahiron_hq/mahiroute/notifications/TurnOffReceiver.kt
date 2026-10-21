// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

package io.github.mahiron_hq.mahiroute.notifications

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import io.github.mahiron_hq.mahiroute.core.EngineHost

/** Handles the "Turn off" action of the status notification. */
class TurnOffReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        EngineHost.start(context)
        EngineHost.setEnabled(false)
    }
}
