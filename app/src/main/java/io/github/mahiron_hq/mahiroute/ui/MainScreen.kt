// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

package io.github.mahiron_hq.mahiroute.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Info
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExposedDropdownMenuAnchorType
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import io.github.mahiron_hq.mahiroute.R
import io.github.mahiron_hq.mahiroute.bootstrap.BootstrapState
import io.github.mahiron_hq.mahiroute.bootstrap.PcStart
import io.github.mahiron_hq.mahiroute.bootstrap.Problem
import io.github.mahiron_hq.mahiroute.bootstrap.Step
import io.github.mahiron_hq.mahiroute.core.AudioDevice
import io.github.mahiron_hq.mahiroute.core.Direction
import io.github.mahiron_hq.mahiroute.core.Link
import io.github.mahiron_hq.mahiroute.core.Routing
import io.github.mahiron_hq.mahiroute.core.UiModel

/** Everything the main screen can ask the rest of the app to do. */
class MainActions(
    val onToggleEnabled: (Boolean) -> Unit,
    val onChoose: (Direction, AudioDevice?) -> Unit,
    val onDismissMissing: (Direction) -> Unit,
    val onSetStartOnBoot: (Boolean) -> Unit,
    val onStartHelper: () -> Unit,
    val onCancelHelper: () -> Unit,
    val onSubmitCode: (String) -> Unit,
    val onOpenDeveloperOptions: () -> Unit,
    val pcStart: () -> PcStart.Info,
    val onConnectPc: (PcStart.Info) -> Unit,
    val onCopy: (String) -> Unit,
    val onSaveLauncher: () -> Unit,
    val onOpenDiagnostics: () -> Unit,
    val onOpenSettings: () -> Unit,
)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MainScreen(state: UiModel, bootstrap: BootstrapState, startOnBoot: Boolean, actions: MainActions) {
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.app_name)) },
                actions = {
                    IconButton(onClick = actions.onOpenDiagnostics) {
                        Icon(Icons.Filled.Info, contentDescription = stringResource(R.string.open_diagnostics))
                    }
                    IconButton(onClick = actions.onOpenSettings) {
                        Icon(Icons.Filled.Settings, contentDescription = stringResource(R.string.open_settings))
                    }
                },
            )
        },
    ) { padding ->
        LazyColumn(
            modifier = Modifier.padding(padding).imePadding(),
            contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 32.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            item { AudioCard(state, actions) }
            if (state.link !is Link.Ready) {
                item { HelperCard(state, bootstrap, actions) }
            }
            items(state.warnedMissing, key = { it.wire }) { direction ->
                val context = LocalContext.current
                val remembered = state.choice(direction)?.let { DeviceLabels.label(context, it) }.orEmpty()
                Banner(
                    stringResource(
                        if (direction == Direction.Input) R.string.missing_input else R.string.missing_output,
                        remembered,
                    ),
                    onClose = { actions.onDismissMissing(direction) },
                )
            }
            item { StartupCard(startOnBoot, actions.onSetStartOnBoot) }
        }
    }
}

@Composable
private fun AudioCard(state: UiModel, actions: MainActions) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        shape = MaterialTheme.shapes.large,
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainerHigh),
    ) {
        Column(Modifier.padding(20.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text(
                        stringResource(if (state.enabled) R.string.switch_enabled else R.string.switch_disabled),
                        style = MaterialTheme.typography.titleMedium,
                    )
                    Text(
                        stringResource(if (state.enabled) R.string.switch_enabled_hint_on else R.string.switch_enabled_hint_off),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                Switch(checked = state.enabled, onCheckedChange = actions.onToggleEnabled)
            }
            DeviceDropdown(R.string.label_input, Direction.Input, state, actions.onChoose)
            DeviceDropdown(R.string.label_output, Direction.Output, state, actions.onChoose)
            RoutingStatusRow(state)
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun DeviceDropdown(
    labelRes: Int,
    direction: Direction,
    state: UiModel,
    onChoose: (Direction, AudioDevice?) -> Unit,
) {
    val context = LocalContext.current
    var expanded by remember { mutableStateOf(false) }
    // A remembered device that is unplugged is not in effect, so the field shows what really applies.
    val chosen = state.chosenDevice(direction)
    val text = chosen?.let { DeviceLabels.label(context, it) } ?: stringResource(R.string.option_system_default)
    ExposedDropdownMenuBox(expanded = expanded, onExpandedChange = { expanded = it }) {
        OutlinedTextField(
            value = text,
            onValueChange = {},
            readOnly = true,
            maxLines = 2,
            enabled = state.enabled,
            label = { Text(stringResource(labelRes)) },
            trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded = expanded) },
            modifier = Modifier.menuAnchor(ExposedDropdownMenuAnchorType.PrimaryNotEditable).fillMaxWidth(),
        )
        ExposedDropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
            DropdownMenuItem(
                text = { Text(stringResource(R.string.option_system_default)) },
                leadingIcon = { if (chosen == null) Icon(Icons.Filled.Check, contentDescription = null) },
                onClick = {
                    expanded = false
                    onChoose(direction, null)
                },
            )
            state.devicesFor(direction).forEach { device ->
                DropdownMenuItem(
                    text = { Text(DeviceLabels.label(context, device)) },
                    leadingIcon = { if (device.key == chosen?.key) Icon(Icons.Filled.Check, contentDescription = null) },
                    onClick = {
                        expanded = false
                        onChoose(direction, device)
                    },
                )
            }
        }
    }
}

@Composable
private fun RoutingStatusRow(state: UiModel) {
    val text =
        when (val routing = state.routing) {
            Routing.Inactive -> stringResource(R.string.routing_inactive)
            Routing.Applying -> stringResource(R.string.routing_applying)
            Routing.Verifying -> stringResource(R.string.routing_verifying)
            Routing.Verified -> stringResource(R.string.routing_verified)
            Routing.Unverified -> stringResource(R.string.routing_unverified)
            is Routing.Failed ->
                stringResource(if (routing.direction == Direction.Input) R.string.routing_failed_input else R.string.routing_failed_output)
        }
    val failed = state.routing is Routing.Failed
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        when (state.routing) {
            Routing.Applying, Routing.Verifying -> CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp)
            Routing.Verified -> Icon(Icons.Filled.Check, null, Modifier.size(18.dp), tint = MaterialTheme.colorScheme.primary)
            is Routing.Failed, Routing.Unverified ->
                Icon(Icons.Filled.Warning, null, Modifier.size(18.dp), tint = MaterialTheme.colorScheme.error)
            Routing.Inactive -> Unit
        }
        Text(
            text,
            style = MaterialTheme.typography.bodySmall,
            color = if (failed) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun StartupCard(startOnBoot: Boolean, onChange: (Boolean) -> Unit) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        shape = MaterialTheme.shapes.large,
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainer),
    ) {
        Row(Modifier.padding(20.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Column(Modifier.weight(1f)) {
                Text(stringResource(R.string.setting_boot_title), style = MaterialTheme.typography.titleMedium)
                Text(
                    stringResource(if (startOnBoot) R.string.setting_boot_hint_on else R.string.setting_boot_hint_off),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Switch(checked = startOnBoot, onCheckedChange = onChange)
        }
    }
}

@Composable
private fun HelperCard(state: UiModel, bootstrap: BootstrapState, actions: MainActions) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        shape = MaterialTheme.shapes.large,
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.tertiaryContainer),
    ) {
        Column(Modifier.padding(20.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            when (bootstrap) {
                is BootstrapState.NeedsPairing -> PairingContent(bootstrap, actions)
                is BootstrapState.Working -> WorkingContent(bootstrap.step, actions.onCancelHelper, actions.onOpenDeveloperOptions)
                else -> StartContent(state, bootstrap, actions)
            }
        }
    }
}

@Composable
private fun StartContent(state: UiModel, bootstrap: BootstrapState, actions: MainActions) {
    Text(stringResource(R.string.helper_title), style = MaterialTheme.typography.titleMedium)
    Text(stringResource(R.string.helper_body), style = MaterialTheme.typography.bodyMedium)
    val failed = bootstrap as? BootstrapState.Failed
    val problem = failed?.problem
    when {
        problem != null -> {
            Text(problemText(problem), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.error)
            failed?.detail?.takeIf { problem == Problem.Other || problem == Problem.Unreachable }?.let {
                Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
        state.link is Link.Failed ->
            Text(
                stringResource(R.string.helper_failed, state.link.reason),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.error,
            )
        else -> Unit
    }
    val connecting = state.link is Link.Connecting
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Button(onClick = actions.onStartHelper, enabled = !connecting) {
            if (connecting) {
                CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp)
                Spacer(Modifier.size(8.dp))
                Text(stringResource(R.string.helper_connecting))
            } else {
                Text(stringResource(R.string.helper_start))
            }
        }
        if (problem == Problem.WirelessDebuggingOff) {
            TextButton(onClick = actions.onOpenDeveloperOptions) { Text(stringResource(R.string.pair_open_settings)) }
        }
        var showPc by rememberSaveable { mutableStateOf(problem == Problem.NoWifi) }
        TextButton(onClick = { showPc = !showPc }) { Text(stringResource(R.string.pc_toggle)) }
        if (showPc) PcContent(actions)
    }
}

@Composable
private fun PcContent(actions: MainActions) {
    val info = remember { actions.pcStart() }
    Text(stringResource(R.string.pc_hint), style = MaterialTheme.typography.bodyMedium)
    Button(onClick = actions.onSaveLauncher) { Text(stringResource(R.string.pc_save_launcher)) }
    Text(stringResource(R.string.pc_manual_hint), style = MaterialTheme.typography.bodySmall)
    SelectionContainer {
        Text(
            info.command,
            style = MaterialTheme.typography.bodySmall.copy(fontFamily = FontFamily.Monospace),
            modifier = Modifier.fillMaxWidth().background(MaterialTheme.colorScheme.surface.copy(alpha = 0.4f), MaterialTheme.shapes.small).padding(10.dp),
        )
    }
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        OutlinedButton(onClick = { actions.onCopy(info.command) }) { Text(stringResource(R.string.pc_copy)) }
        OutlinedButton(onClick = { actions.onConnectPc(info) }) { Text(stringResource(R.string.pc_connect)) }
    }
}

@Composable
private fun WorkingContent(step: Step, onCancel: () -> Unit, onOpenDeveloperOptions: () -> Unit) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp)
        Text(
            stringResource(
                when (step) {
                    Step.Preparing -> R.string.helper_step_preparing
                    Step.WaitingForDebugging -> R.string.helper_step_debugging
                    Step.FindingService -> R.string.helper_step_finding
                    Step.WaitingForConfirmation -> R.string.helper_step_confirm
                    Step.StartingHelper -> R.string.helper_step_starting
                    Step.FindingPairingDialog -> R.string.helper_step_dialog
                    Step.Pairing -> R.string.helper_step_pairing
                },
            ),
            style = MaterialTheme.typography.bodyLarge,
            modifier = Modifier.weight(1f),
        )
        TextButton(onClick = onCancel) { Text(stringResource(R.string.cancel)) }
    }
    if (step == Step.WaitingForDebugging) {
        TextButton(onClick = onOpenDeveloperOptions) { Text(stringResource(R.string.pair_open_settings)) }
    }
}

@Composable
private fun PairingContent(pairing: BootstrapState.NeedsPairing, actions: MainActions) {
    var code by rememberSaveable { mutableStateOf("") }
    Text(stringResource(R.string.pair_title), style = MaterialTheme.typography.titleMedium)
    Text(stringResource(R.string.pair_instructions), style = MaterialTheme.typography.bodyMedium)
    if (pairing.lastCodeRejected) {
        Text(stringResource(R.string.pair_wrong_code), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.error)
    }
    if (pairing.dialogMissing) {
        Text(stringResource(R.string.pair_dialog_missing), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.error)
    }
    OutlinedTextField(
        value = code,
        onValueChange = { code = it.filter(Char::isDigit).take(6) },
        singleLine = true,
        label = { Text(stringResource(R.string.pair_code_hint)) },
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number, imeAction = ImeAction.Done),
        keyboardActions = KeyboardActions(onDone = { if (code.length == 6) actions.onSubmitCode(code) }),
        modifier = Modifier.fillMaxWidth(),
    )
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Button(onClick = { actions.onSubmitCode(code) }, enabled = code.length == 6) { Text(stringResource(R.string.pair_submit)) }
        TextButton(onClick = actions.onOpenDeveloperOptions) { Text(stringResource(R.string.pair_open_settings)) }
        TextButton(onClick = actions.onCancelHelper) { Text(stringResource(R.string.cancel)) }
    }
}

@Composable
private fun problemText(problem: Problem): String =
    stringResource(
        when (problem) {
            Problem.NoWifi -> R.string.problem_no_wifi
            Problem.WirelessDebuggingOff -> R.string.problem_debugging_off
            Problem.ServiceNotFound -> R.string.problem_not_found
            Problem.Unreachable -> R.string.problem_unreachable
            Problem.Other -> R.string.problem_other
        },
    )

@Composable
private fun Banner(text: String, onClose: () -> Unit) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.errorContainer),
    ) {
        Row(Modifier.padding(start = 16.dp, top = 4.dp, bottom = 4.dp, end = 4.dp), horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
            Icon(Icons.Filled.Warning, null, tint = MaterialTheme.colorScheme.onErrorContainer)
            Text(text, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onErrorContainer, modifier = Modifier.weight(1f))
            IconButton(onClick = onClose) {
                Icon(Icons.Filled.Close, contentDescription = stringResource(R.string.close), tint = MaterialTheme.colorScheme.onErrorContainer)
            }
        }
    }
}
