// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

package io.github.mahiron_hq.mahiroute.service

import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import android.util.Log
import androidx.core.content.ContextCompat
import io.github.mahiron_hq.mahiroute.bootstrap.BootstrapState
import io.github.mahiron_hq.mahiroute.bootstrap.HelperBootstrap
import io.github.mahiron_hq.mahiroute.core.Direction
import io.github.mahiron_hq.mahiroute.core.EngineHost
import io.github.mahiron_hq.mahiroute.core.Link
import io.github.mahiron_hq.mahiroute.core.UiModel
import io.github.mahiron_hq.mahiroute.notifications.Notifications
import io.github.mahiron_hq.mahiroute.settings.LocaleSupport
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.collect
import kotlinx.coroutines.launch

/**
 * Keeps the process alive while routing is active so the engine can react to devices that come
 * and go, and tells the user when a chosen device disappears or returns.
 */
class RoutingService : Service() {
    override fun attachBaseContext(newBase: Context) {
        super.attachBaseContext(LocaleSupport.wrap(newBase))
    }

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private var previous: UiModel? = null

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        Notifications.ensureChannels(this)
        val notification = Notifications.status(this, EngineHost.state.value)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            startForeground(Notifications.STATUS_ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE)
        } else {
            startForeground(Notifications.STATUS_ID, notification)
        }
        scope.launch { EngineHost.state.collect(::onState) }
        // A dropped helper is restarted automatically; only a failed restart needs the user.
        scope.launch {
            HelperBootstrap.state.collect { bootstrap ->
                if (bootstrap is BootstrapState.Failed) Notifications.helperLost(this@RoutingService)
            }
        }
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int = START_STICKY

    override fun onDestroy() {
        scope.cancel()
        super.onDestroy()
    }

    private fun onState(state: UiModel) {
        val before = previous
        previous = state
        getSystemService(android.app.NotificationManager::class.java)
            .notify(Notifications.STATUS_ID, Notifications.status(this, state))
        for (direction in Direction.entries) {
            val dismissed = direction in state.dismissedMissing
            if (dismissed) Notifications.cancelDeviceAlert(this, direction)
            if (before == null) continue
            val wasMissing = direction in before.missing
            val isMissing = direction in state.missing
            if (isMissing && !wasMissing && !dismissed) Notifications.deviceMissing(this, direction)
            if (wasMissing && !isMissing && state.choice(direction) != null) Notifications.deviceRestored(this, direction)
        }
        if (state.link is Link.Ready) Notifications.cancelHelperLost(this)
    }

    companion object {
        private const val TAG = "MahiRoute"

        /** Whether the engine needs the process to stay alive. */
        fun wanted(state: UiModel): Boolean = state.enabled && (state.input != null || state.output != null)

        fun start(context: Context) {
            try {
                ContextCompat.startForegroundService(context, Intent(context, RoutingService::class.java))
            } catch (error: IllegalStateException) {
                // Android refuses background starts it does not consider user-visible.
                Log.w(TAG, "cannot start the foreground service now", error)
            }
        }

        fun stop(context: Context) {
            context.stopService(Intent(context, RoutingService::class.java))
        }
    }
}
