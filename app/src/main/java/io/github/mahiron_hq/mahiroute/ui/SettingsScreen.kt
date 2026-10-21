// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

package io.github.mahiron_hq.mahiroute.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import io.github.mahiron_hq.mahiroute.R
import io.github.mahiron_hq.mahiroute.settings.LanguageChoice
import io.github.mahiron_hq.mahiroute.settings.ThemeMode

/** Everything the settings screen shows and can change. */
class SettingsActions(
    val language: LanguageChoice,
    val theme: ThemeMode,
    val alerts: Boolean,
    val onLanguage: (LanguageChoice) -> Unit,
    val onTheme: (ThemeMode) -> Unit,
    val onAlerts: (Boolean) -> Unit,
    val onSystemNotifications: () -> Unit,
    val onCopyReport: () -> Unit,
    val onShareReport: () -> Unit,
    val onReset: () -> Unit,
    val onOpenAbout: () -> Unit,
    val onOpenPolicy: () -> Unit,
)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(actions: SettingsActions, onBack: () -> Unit) {
    var confirmReset by rememberSaveable { mutableStateOf(false) }
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.settings_title)) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = stringResource(R.string.back))
                    }
                },
            )
        },
    ) { padding ->
        Column(
            Modifier.padding(padding).verticalScroll(rememberScrollState()).padding(horizontal = 16.dp, vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Heading(R.string.settings_language)
            Choices(LanguageChoice.entries, actions.language, actions.onLanguage) { stringResource(languageName(it)) }
            HorizontalDivider()
            Heading(R.string.settings_theme)
            Choices(ThemeMode.entries, actions.theme, actions.onTheme) { stringResource(themeName(it)) }
            HorizontalDivider()
            Heading(R.string.settings_notifications)
            Row(verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text(stringResource(R.string.alerts_title), style = MaterialTheme.typography.bodyLarge)
                    Text(
                        stringResource(R.string.alerts_text),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                Switch(checked = actions.alerts, onCheckedChange = actions.onAlerts)
            }
            TextButton(onClick = actions.onSystemNotifications) { Text(stringResource(R.string.notif_system_settings)) }
            HorizontalDivider()
            Heading(R.string.settings_support)
            Text(
                stringResource(R.string.report_hint),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                OutlinedButton(onClick = actions.onCopyReport) { Text(stringResource(R.string.report_copy)) }
                OutlinedButton(onClick = actions.onShareReport) { Text(stringResource(R.string.report_share)) }
            }
            OutlinedButton(onClick = { confirmReset = true }) { Text(stringResource(R.string.reset_settings)) }
            HorizontalDivider()
            Heading(R.string.settings_about)
            TextButton(onClick = actions.onOpenPolicy) { Text(stringResource(R.string.privacy_policy)) }
            TextButton(onClick = actions.onOpenAbout) { Text(stringResource(R.string.about_title)) }
        }
    }
    if (confirmReset) {
        AlertDialog(
            onDismissRequest = { confirmReset = false },
            title = { Text(stringResource(R.string.reset_confirm_title)) },
            text = { Text(stringResource(R.string.reset_confirm_text)) },
            confirmButton = {
                Button(
                    onClick = {
                        confirmReset = false
                        actions.onReset()
                    },
                ) {
                    Text(stringResource(R.string.reset_confirm))
                }
            },
            dismissButton = { TextButton(onClick = { confirmReset = false }) { Text(stringResource(R.string.cancel)) } },
        )
    }
}

@Composable
private fun Heading(res: Int) {
    Text(stringResource(res), style = MaterialTheme.typography.titleMedium, modifier = Modifier.padding(top = 8.dp))
}

@Composable
private fun <T> Choices(options: List<T>, selected: T, onSelect: (T) -> Unit, label: @Composable (T) -> String) {
    Column(Modifier.fillMaxWidth()) {
        options.forEach { option ->
            Row(
                Modifier.fillMaxWidth().selectable(selected = option == selected, role = Role.RadioButton) { onSelect(option) },
                verticalAlignment = Alignment.CenterVertically,
            ) {
                RadioButton(selected = option == selected, onClick = null)
                Text(label(option), modifier = Modifier.padding(start = 12.dp, top = 12.dp, bottom = 12.dp))
            }
        }
    }
}

private fun languageName(choice: LanguageChoice) =
    when (choice) {
        LanguageChoice.System -> R.string.language_system
        LanguageChoice.Russian -> R.string.language_ru
        LanguageChoice.English -> R.string.language_en
    }

private fun themeName(mode: ThemeMode) =
    when (mode) {
        ThemeMode.System -> R.string.theme_system
        ThemeMode.Light -> R.string.theme_light
        ThemeMode.Dark -> R.string.theme_dark
    }
