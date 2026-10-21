// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

package io.github.mahiron_hq.mahiroute.notifications

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.RemoteInput
import android.content.Context
import android.content.Intent
import io.github.mahiron_hq.mahiroute.MainActivity
import io.github.mahiron_hq.mahiroute.R
import io.github.mahiron_hq.mahiroute.core.Direction
import io.github.mahiron_hq.mahiroute.core.UiModel
import io.github.mahiron_hq.mahiroute.ui.DeviceLabels

object Notifications {
    const val STATUS_ID = 1
    private const val CHANNEL_STATUS = "status"
    private const val CHANNEL_ALERTS = "alerts"
    private const val ALERT_MISSING_BASE = 10
    private const val ALERT_HELPER = 12
    private const val PAIRING_ID = 3
    const val REMOTE_INPUT_KEY = "pairing_code"

    fun ensureChannels(context: Context) {
        val manager = context.getSystemService(NotificationManager::class.java)
        manager.createNotificationChannel(
            NotificationChannel(CHANNEL_STATUS, context.getString(R.string.channel_status), NotificationManager.IMPORTANCE_LOW),
        )
        manager.createNotificationChannel(
            NotificationChannel(CHANNEL_ALERTS, context.getString(R.string.channel_alerts), NotificationManager.IMPORTANCE_HIGH),
        )
    }

    /** The ongoing notification shown while the service keeps the routing alive. */
    fun status(context: Context, state: UiModel): Notification {
        val systemDefault = context.getString(R.string.option_system_default)
        fun chosen(direction: Direction): String =
            state.chosenDevice(direction)?.let { DeviceLabels.label(context, it) } ?: systemDefault
        return Notification.Builder(context, CHANNEL_STATUS)
            .setSmallIcon(R.drawable.ic_stat_route)
            .setContentTitle(context.getString(R.string.notif_status_title))
            .setContentText(context.getString(R.string.notif_status_text, chosen(Direction.Input), chosen(Direction.Output)))
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setContentIntent(openApp(context))
            .addAction(
                Notification.Action.Builder(null, context.getString(R.string.notif_action_off), turnOff(context)).build(),
            )
            .build()
    }

    fun deviceMissing(context: Context, direction: Direction) =
        notify(
            context,
            ALERT_MISSING_BASE + direction.ordinal,
            context.getString(R.string.notif_missing_title),
            context.getString(R.string.notif_missing_text, DeviceLabels.directionNoun(context, direction)),
        )

    fun deviceRestored(context: Context, direction: Direction) =
        notify(
            context,
            ALERT_MISSING_BASE + direction.ordinal,
            context.getString(R.string.notif_restored_title),
            context.getString(R.string.notif_restored_text, DeviceLabels.directionNoun(context, direction)),
        )

    /** Removes the "disconnected" or "reconnected" alert of one direction, e.g. after it was closed. */
    fun cancelDeviceAlert(context: Context, direction: Direction) {
        context.getSystemService(NotificationManager::class.java).cancel(ALERT_MISSING_BASE + direction.ordinal)
    }

    fun cancelHelperLost(context: Context) {
        context.getSystemService(NotificationManager::class.java).cancel(ALERT_HELPER)
    }

    fun helperLost(context: Context) =
        notify(
            context,
            ALERT_HELPER,
            context.getString(R.string.notif_helper_lost_title),
            context.getString(R.string.notif_helper_lost_text),
        )

    /** Asks for the pairing code in a notification the user can answer without leaving the settings. */
    fun showPairingPrompt(context: Context) {
        ensureChannels(context)
        val input = RemoteInput.Builder(REMOTE_INPUT_KEY).setLabel(context.getString(R.string.pair_code_hint)).build()
        // A reply must be able to carry the typed text, so the intent has to be mutable.
        val reply =
            PendingIntent.getBroadcast(
                context,
                2,
                Intent(context, PairingReceiver::class.java),
                PendingIntent.FLAG_MUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
            )
        val action =
            Notification.Action.Builder(null, context.getString(R.string.notif_pair_action), reply).addRemoteInput(input).build()
        val notification =
            Notification.Builder(context, CHANNEL_ALERTS)
                .setSmallIcon(R.drawable.ic_stat_route)
                .setContentTitle(context.getString(R.string.notif_pair_title))
                .setContentText(context.getString(R.string.notif_pair_text))
                .setOnlyAlertOnce(true)
                .setContentIntent(openApp(context))
                .addAction(action)
                .build()
        context.getSystemService(NotificationManager::class.java).notify(PAIRING_ID, notification)
    }

    fun cancelPairingPrompt(context: Context) {
        context.getSystemService(NotificationManager::class.java).cancel(PAIRING_ID)
    }

    private fun notify(context: Context, id: Int, title: String, text: String) {
        ensureChannels(context)
        val notification =
            Notification.Builder(context, CHANNEL_ALERTS)
                .setSmallIcon(R.drawable.ic_stat_route)
                .setContentTitle(title)
                .setContentText(text)
                .setAutoCancel(true)
                .setContentIntent(openApp(context))
                .build()
        context.getSystemService(NotificationManager::class.java).notify(id, notification)
    }

    private fun openApp(context: Context): PendingIntent =
        PendingIntent.getActivity(
            context,
            0,
            Intent(context, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )

    private fun turnOff(context: Context): PendingIntent =
        PendingIntent.getBroadcast(
            context,
            1,
            Intent(context, TurnOffReceiver::class.java),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
}
