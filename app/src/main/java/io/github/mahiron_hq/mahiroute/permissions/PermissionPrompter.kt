// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

package io.github.mahiron_hq.mahiroute.permissions

import android.content.Context
import android.content.Intent
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.res.stringResource
import androidx.core.content.edit
import io.github.mahiron_hq.mahiroute.R

/**
 * Asks for permissions at launch and again whenever the user touches something that needs a
 * permission they refused.
 */
@Stable
class PermissionPrompter internal constructor(
    private val activity: ComponentActivity,
    private val launchRuntime: (Array<String>) -> Unit,
    private val launchSettings: (Intent) -> Unit,
) {
    private var pending by mutableStateOf<AppPermission?>(null)

    /** True when [permission] is granted; otherwise opens the explanation dialog and returns false. */
    fun ensure(permission: AppPermission): Boolean {
        if (Permissions.isGranted(activity, permission)) return true
        pending = permission
        return false
    }

    /** Requests every missing runtime permission once, on the very first launch. */
    fun requestAtLaunch() {
        val prefs = activity.getSharedPreferences(LAUNCH_PREFS, Context.MODE_PRIVATE)
        if (prefs.getBoolean(KEY_DONE, false)) return
        prefs.edit { putBoolean(KEY_DONE, true) }
        val missing =
            listOf(AppPermission.Notifications, AppPermission.Bluetooth)
                .filter { !Permissions.isGranted(activity, it) && Permissions.runtimeNames(it).isNotEmpty() }
        if (missing.isEmpty()) {
            offerBatteryExemption()
            return
        }
        Permissions.markAsked(activity, missing)
        launchRuntime(missing.flatMap { Permissions.runtimeNames(it).asList() }.toTypedArray())
    }

    internal fun onRuntimeResult() = offerBatteryExemption()

    private fun offerBatteryExemption() {
        if (!Permissions.isGranted(activity, AppPermission.Battery)) pending = AppPermission.Battery
    }

    private fun grant(permission: AppPermission) {
        pending = null
        val names = Permissions.runtimeNames(permission)
        if (names.isNotEmpty() && Permissions.canAsk(activity, permission)) {
            Permissions.markAsked(activity, listOf(permission))
            launchRuntime(names)
        } else {
            launchSettings(Permissions.settingsIntent(activity, permission))
        }
    }

    @Composable
    fun Dialog() {
        val permission = pending ?: return
        val canAsk = Permissions.runtimeNames(permission).isEmpty() || Permissions.canAsk(activity, permission)
        AlertDialog(
            onDismissRequest = { pending = null },
            title = { Text(stringResource(R.string.perm_title)) },
            text = {
                Text(
                    stringResource(
                        when (permission) {
                            AppPermission.Notifications -> R.string.perm_notifications
                            AppPermission.Bluetooth -> R.string.perm_bluetooth
                            AppPermission.Battery -> R.string.perm_battery
                        },
                    ),
                )
            },
            confirmButton = {
                TextButton(onClick = { grant(permission) }) {
                    Text(stringResource(if (canAsk) R.string.perm_grant else R.string.perm_open_settings))
                }
            },
            dismissButton = { TextButton(onClick = { pending = null }) { Text(stringResource(R.string.perm_later)) } },
        )
    }

    private companion object {
        const val LAUNCH_PREFS = "launch"
        const val KEY_DONE = "permissions_requested"
    }
}

@Composable
fun rememberPermissionPrompter(activity: ComponentActivity): PermissionPrompter {
    var prompter by remember { mutableStateOf<PermissionPrompter?>(null) }
    val runtime =
        rememberLauncherForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) {
            prompter?.onRuntimeResult()
        }
    val settings = rememberLauncherForActivityResult(ActivityResultContracts.StartActivityForResult()) {}
    return remember(activity) {
        PermissionPrompter(activity, { runtime.launch(it) }, { settings.launch(it) }).also { prompter = it }
    }
}
