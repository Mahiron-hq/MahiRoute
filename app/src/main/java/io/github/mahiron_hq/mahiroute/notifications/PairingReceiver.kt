// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

package io.github.mahiron_hq.mahiroute.notifications

import android.app.RemoteInput
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import io.github.mahiron_hq.mahiroute.bootstrap.HelperBootstrap

/** Receives the pairing code typed into the notification. */
class PairingReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val code = RemoteInput.getResultsFromIntent(intent)?.getCharSequence(Notifications.REMOTE_INPUT_KEY)?.toString()
        if (code != null) HelperBootstrap.submitPairingCode(code)
    }
}
