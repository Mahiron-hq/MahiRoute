// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

package io.github.mahiron_hq.mahiroute

import android.content.ActivityNotFoundException
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
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
import io.github.mahiron_hq.mahiroute.core.Direction
import io.github.mahiron_hq.mahiroute.core.EngineHost
import io.github.mahiron_hq.mahiroute.core.ReportText
import io.github.mahiron_hq.mahiroute.core.UiModel
import io.github.mahiron_hq.mahiroute.permissions.AppPermission
import io.github.mahiron_hq.mahiroute.permissions.rememberPermissionPrompter
import io.github.mahiron_hq.mahiroute.settings.AppSettings
import io.github.mahiron_hq.mahiroute.settings.LanguageChoice
import io.github.mahiron_hq.mahiroute.settings.LocaleSupport
import io.github.mahiron_hq.mahiroute.settings.needsConsent
import io.github.mahiron_hq.mahiroute.startup.StartupPolicy
import io.github.mahiron_hq.mahiroute.ui.AboutInfo
import io.github.mahiron_hq.mahiroute.ui.AboutScreen
import io.github.mahiron_hq.mahiroute.ui.ConsentScreen
import io.github.mahiron_hq.mahiroute.ui.DeviceLabels
import io.github.mahiron_hq.mahiroute.ui.DiagnosticsScreen
import io.github.mahiron_hq.mahiroute.ui.MahiRouteTheme
import io.github.mahiron_hq.mahiroute.ui.MainActions
import io.github.mahiron_hq.mahiroute.ui.MainScreen
import io.github.mahiron_hq.mahiroute.ui.PolicyScreen
import io.github.mahiron_hq.mahiroute.ui.SettingsActions
import io.github.mahiron_hq.mahiroute.ui.SettingsScreen
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

private enum class Screen { Main, Diagnostics, Settings, About, Policy }

class MainActivity : ComponentActivity() {
    override fun attachBaseContext(newBase: Context) {
        super.attachBaseContext(LocaleSupport.wrap(newBase))
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        handleDebugIntent(intent)
        // The script a computer runs through adb must exist on the phone before anyone asks for it.
        lifecycleScope.launch(Dispatchers.IO) { PcStart.prepare(this@MainActivity) }
        setContent {
            val theme by AppSettings.theme.collectAsStateWithLifecycle()
            MahiRouteTheme(theme) { Root() }
        }
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
        val accepted by AppSettings.acceptedPolicy.collectAsStateWithLifecycle()
        val language by AppSettings.language.collectAsStateWithLifecycle()
        val theme by AppSettings.theme.collectAsStateWithLifecycle()
        val alerts by AppSettings.alerts.collectAsStateWithLifecycle()
        val prompter = rememberPermissionPrompter(this)
        var screen by rememberSaveable { mutableStateOf(Screen.Main) }
        // Permission prompts wait until the privacy notice is accepted.
        val consented = !needsConsent(accepted, AppSettings.POLICY_VERSION)
        LaunchedEffect(consented) { if (consented) prompter.requestAtLaunch() }
        BackHandler(enabled = screen != Screen.Main && consented) {
            screen = if (screen == Screen.Policy || screen == Screen.About) Screen.Settings else Screen.Main
        }

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
                    onOpenDiagnostics = { screen = Screen.Diagnostics },
                    onOpenSettings = { screen = Screen.Settings },
                )
            }

        val settings =
            SettingsActions(
                language = language,
                theme = theme,
                alerts = alerts,
                onLanguage = { choice ->
                    AppSettings.setLanguage(this, choice)
                    recreate()
                },
                onTheme = { AppSettings.setTheme(this, it) },
                onAlerts = { AppSettings.setAlerts(this, it) },
                onSystemNotifications = ::openNotificationSettings,
                onCopyReport = {
                    copyToClipboard(report(state))
                    Toast.makeText(this, getString(R.string.report_copied), Toast.LENGTH_SHORT).show()
                },
                onShareReport = { shareReport(report(state)) },
                onReset = {
                    EngineHost.choose(Direction.Input, null)
                    EngineHost.choose(Direction.Output, null)
                    EngineHost.setEnabled(false)
                    val languageChanged = language != LanguageChoice.System
                    AppSettings.reset(this)
                    if (languageChanged) recreate()
                },
                onOpenAbout = { screen = Screen.About },
                onOpenPolicy = { screen = Screen.Policy },
            )

        when {
            !consented ->
                ConsentScreen(
                    updated = accepted > 0,
                    onAccept = { AppSettings.acceptPolicy(this) },
                    onDecline = ::finishAndRemoveTask,
                )
            screen == Screen.Diagnostics ->
                DiagnosticsScreen(state, onBack = { screen = Screen.Main }, onRecalibrate = EngineHost::recalibrate)
            screen == Screen.Settings -> SettingsScreen(settings, onBack = { screen = Screen.Main })
            screen == Screen.Policy -> PolicyScreen(onBack = { screen = Screen.Settings })
            screen == Screen.About ->
                AboutScreen(
                    AboutInfo(BuildConfig.VERSION_NAME, BuildConfig.VERSION_CODE, ::openLink) { screen = Screen.Policy },
                    onBack = { screen = Screen.Settings },
                )
            else -> MainScreen(state, bootstrap, startOnBoot, actions)
        }
        if (consented) prompter.Dialog()
    }

    private fun report(state: UiModel): String =
        ReportText.build(
            state,
            "${BuildConfig.VERSION_NAME} (${BuildConfig.VERSION_CODE})",
            Build.VERSION.RELEASE,
            "${Build.MANUFACTURER} ${Build.MODEL}",
        )

    private fun shareReport(text: String) {
        val send =
            Intent(Intent.ACTION_SEND)
                .setType("text/plain")
                .putExtra(Intent.EXTRA_SUBJECT, getString(R.string.report_share_title))
                .putExtra(Intent.EXTRA_TEXT, text)
        startActivity(Intent.createChooser(send, getString(R.string.report_share_title)))
    }

    private fun openNotificationSettings() {
        startActivity(Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS).putExtra(Settings.EXTRA_APP_PACKAGE, packageName))
    }

    private fun openLink(url: String) {
        try {
            startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)))
        } catch (_: ActivityNotFoundException) {
            Toast.makeText(this, getString(R.string.about_open_failed), Toast.LENGTH_SHORT).show()
        }
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
