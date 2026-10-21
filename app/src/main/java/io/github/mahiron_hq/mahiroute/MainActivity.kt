// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

package io.github.mahiron_hq.mahiroute

import android.content.ActivityNotFoundException
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Intent
import android.os.Bundle
import android.provider.Settings
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.lifecycleScope
import io.github.mahiron_hq.mahiroute.bootstrap.HelperBootstrap
import io.github.mahiron_hq.mahiroute.bootstrap.PcStart
import io.github.mahiron_hq.mahiroute.core.EngineHost
import io.github.mahiron_hq.mahiroute.permissions.AppPermission
import io.github.mahiron_hq.mahiroute.permissions.rememberPermissionPrompter
import io.github.mahiron_hq.mahiroute.startup.StartupPolicy
import io.github.mahiron_hq.mahiroute.ui.DeviceLabels
import io.github.mahiron_hq.mahiroute.ui.DiagnosticsScreen
import io.github.mahiron_hq.mahiroute.ui.MahiRouteTheme
import io.github.mahiron_hq.mahiroute.ui.MainActions
import io.github.mahiron_hq.mahiroute.ui.MainScreen
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        handleDebugIntent(intent)
        // The script a computer runs through adb must exist on the phone before anyone asks for it.
        lifecycleScope.launch(Dispatchers.IO) { PcStart.prepare(this@MainActivity) }
        setContent { MahiRouteTheme { Root() } }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handleDebugIntent(intent)
    }

    @Composable
    private fun Root() {
        val state by EngineHost.state.collectAsStateWithLifecycle()
        val bootstrap by HelperBootstrap.state.collectAsStateWithLifecycle()
        val startOnBoot by StartupPolicy.startOnBoot.collectAsStateWithLifecycle()
        val prompter = rememberPermissionPrompter(this)
        var showDiagnostics by rememberSaveable { mutableStateOf(false) }
        LaunchedEffect(Unit) { prompter.requestAtLaunch() }
        BackHandler(enabled = showDiagnostics) { showDiagnostics = false }

        val actions =
            remember(prompter) {
                MainActions(
                    onToggleEnabled = { enabled ->
                        if (enabled) prompter.ensure(AppPermission.Notifications)
                        EngineHost.setEnabled(enabled)
                    },
                    onChoose = { direction, device ->
                        val needsBluetooth = device != null && DeviceLabels.isBluetooth(device.kind)
                        if (!needsBluetooth || prompter.ensure(AppPermission.Bluetooth)) {
                            EngineHost.choose(direction, device?.key)
                        }
                    },
                    onDismissMissing = EngineHost::dismissMissing,
                    onSetStartOnBoot = { StartupPolicy.setStartOnBoot(this, it) },
                    onStartHelper = { HelperBootstrap.start(this) },
                    onCancelHelper = { HelperBootstrap.cancel(this) },
                    onSubmitCode = HelperBootstrap::submitPairingCode,
                    onOpenDeveloperOptions = ::openDeveloperOptions,
                    pcStart = { PcStart.prepare(this) },
                    onConnectPc = { PcStart.connect(this, it) },
                    onCopy = ::copyToClipboard,
                    onSaveLauncher = ::saveWindowsLauncher,
                    onOpenDiagnostics = { showDiagnostics = true },
                )
            }

        if (showDiagnostics) {
            DiagnosticsScreen(state, onBack = { showDiagnostics = false }, onRecalibrate = EngineHost::recalibrate)
        } else {
            MainScreen(state, bootstrap, startOnBoot, actions)
        }
        prompter.Dialog()
    }

    private fun copyToClipboard(text: String) {
        getSystemService(ClipboardManager::class.java).setPrimaryClip(ClipData.newPlainText("adb", text))
    }

    private fun saveWindowsLauncher() {
        lifecycleScope.launch {
            val saved = withContext(Dispatchers.IO) { PcStart.saveWindowsLauncher(this@MainActivity) }
            Toast.makeText(
                this@MainActivity,
                getString(if (saved) R.string.pc_launcher_saved else R.string.pc_launcher_failed),
                Toast.LENGTH_LONG,
            ).show()
        }
    }

    private fun openDeveloperOptions() {
        try {
            startActivity(Intent(Settings.ACTION_APPLICATION_DEVELOPMENT_SETTINGS))
        } catch (_: ActivityNotFoundException) {
            startActivity(Intent(Settings.ACTION_SETTINGS))
        }
    }

    /** Debug builds accept a helper endpoint through intent extras, for scripted device tests. */
    private fun handleDebugIntent(intent: Intent) {
        if (!BuildConfig.DEBUG) return
        val port = intent.getIntExtra("helper_port", -1)
        val token = intent.getStringExtra("helper_token")
        if (port > 0 && token != null) EngineHost.connect(port, token)
    }
}
