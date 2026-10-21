// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

package io.github.mahiron_hq.mahiroute.permissions

import android.Manifest
import android.annotation.SuppressLint
import android.app.Activity
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.PowerManager
import android.provider.Settings
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import androidx.core.content.edit

/** Permissions the app asks for, in the order of importance. */
enum class AppPermission {
    Notifications,
    Bluetooth,
    Battery,
}

object Permissions {
    private const val PREFS = "permissions"

    fun isGranted(context: Context, permission: AppPermission): Boolean =
        when (permission) {
            AppPermission.Notifications ->
                Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU ||
                    granted(context, Manifest.permission.POST_NOTIFICATIONS)
            AppPermission.Bluetooth -> granted(context, Manifest.permission.BLUETOOTH_CONNECT)
            AppPermission.Battery ->
                context.getSystemService(PowerManager::class.java).isIgnoringBatteryOptimizations(context.packageName)
        }

    /** Runtime permission names behind [permission]; empty for the battery exemption. */
    fun runtimeNames(permission: AppPermission): Array<String> =
        when (permission) {
            AppPermission.Notifications ->
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                    arrayOf(Manifest.permission.POST_NOTIFICATIONS)
                } else {
                    emptyArray()
                }
            AppPermission.Bluetooth -> arrayOf(Manifest.permission.BLUETOOTH_CONNECT)
            AppPermission.Battery -> emptyArray()
        }

    /** True when the system will still show its permission dialog for [permission]. */
    fun canAsk(activity: Activity, permission: AppPermission): Boolean {
        val names = runtimeNames(permission)
        if (names.isEmpty()) return true
        val askedBefore = activity.getSharedPreferences(PREFS, Context.MODE_PRIVATE).getBoolean(permission.name, false)
        return !askedBefore || names.any { ActivityCompat.shouldShowRequestPermissionRationale(activity, it) }
    }

    fun markAsked(context: Context, permissions: Collection<AppPermission>) {
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit {
            permissions.forEach { putBoolean(it.name, true) }
        }
    }

    /**
     * Intent that lets the user grant [permission] in system settings. The battery exemption is
     * requested although Google Play restricts it: the app is distributed elsewhere and the user may decline.
     */
    @SuppressLint("BatteryLife")
    fun settingsIntent(context: Context, permission: AppPermission): Intent =
        when (permission) {
            AppPermission.Notifications ->
                Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS).putExtra(Settings.EXTRA_APP_PACKAGE, context.packageName)
            AppPermission.Bluetooth ->
                Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.fromParts("package", context.packageName, null))
            AppPermission.Battery ->
                Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS, Uri.fromParts("package", context.packageName, null))
        }

    private fun granted(context: Context, name: String) =
        ContextCompat.checkSelfPermission(context, name) == PackageManager.PERMISSION_GRANTED
}
