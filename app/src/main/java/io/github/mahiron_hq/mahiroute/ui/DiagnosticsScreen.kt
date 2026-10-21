// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

package io.github.mahiron_hq.mahiroute.ui

import android.content.Context
import android.os.Build
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import io.github.mahiron_hq.mahiroute.R
import io.github.mahiron_hq.mahiroute.core.AudioDevice
import io.github.mahiron_hq.mahiroute.core.Direction
import io.github.mahiron_hq.mahiroute.core.Link
import io.github.mahiron_hq.mahiroute.core.Stream
import io.github.mahiron_hq.mahiroute.core.Support
import io.github.mahiron_hq.mahiroute.core.UiModel

/** Shows what the audio system is really doing, so the routing can be checked by eye. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DiagnosticsScreen(state: UiModel, onBack: () -> Unit, onRecalibrate: () -> Unit) {
    val context = LocalContext.current
    val packages = remember { PackageNames(context) }
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.diag_title)) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = stringResource(R.string.back))
                    }
                },
            )
        },
    ) { padding ->
        LazyColumn(
            modifier = Modifier.padding(padding),
            contentPadding = PaddingValues(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            item { HelperSection(state, onRecalibrate) }
            item { SectionTitle(R.string.diag_devices) }
            items(state.devices) { device -> DeviceRow(context, device) }
            if (state.warnings.isNotEmpty()) {
                item { SectionTitle(R.string.diag_warnings) }
                items(state.warnings) { Muted(it) }
            }
            item { SectionTitle(R.string.diag_streams) }
            if (state.streams.isEmpty()) {
                item { Muted(stringResource(R.string.diag_no_streams)) }
            }
            items(state.streams) { stream -> StreamRow(context, stream, state, packages) }
        }
    }
}

@Composable
private fun HelperSection(state: UiModel, onRecalibrate: () -> Unit) {
    Card(colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainerHigh)) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            val link =
                when (val current = state.link) {
                    Link.Ready -> stringResource(R.string.diag_link_ready)
                    Link.Connecting -> stringResource(R.string.diag_link_connecting)
                    Link.Disconnected -> stringResource(R.string.diag_link_disconnected)
                    is Link.Failed -> stringResource(R.string.diag_link_failed, current.reason)
                }
            Text(stringResource(R.string.diag_helper), style = MaterialTheme.typography.titleSmall)
            Text(link, style = MaterialTheme.typography.bodyMedium)
            state.sdk?.let {
                Text(
                    stringResource(R.string.diag_firmware, Build.VERSION.RELEASE_OR_CODENAME, it),
                    style = MaterialTheme.typography.bodyMedium,
                )
            }
            Text(stringResource(R.string.diag_mode, modeName(state.mode)), style = MaterialTheme.typography.bodyMedium)
            Text(stringResource(R.string.diag_capabilities), style = MaterialTheme.typography.titleSmall, modifier = Modifier.padding(top = 8.dp))
            Text("${stringResource(R.string.diag_capture_preset)}: ${support(state.capturePreset)}", style = MaterialTheme.typography.bodyMedium)
            Text("${stringResource(R.string.diag_output_strategy)}: ${support(state.outputStrategy)}", style = MaterialTheme.typography.bodyMedium)
            Button(onClick = onRecalibrate, enabled = state.link is Link.Ready, modifier = Modifier.padding(top = 8.dp)) {
                Text(stringResource(R.string.diag_recalibrate))
            }
        }
    }
}

@Composable
private fun support(value: Support): String =
    stringResource(
        when (value) {
            Support.Unknown -> R.string.support_unknown
            Support.Honored -> R.string.support_honored
            Support.Ignored -> R.string.support_ignored
        },
    )

@Composable
private fun modeName(mode: Int): String =
    stringResource(
        when (mode) {
            1 -> R.string.mode_ringtone
            2 -> R.string.mode_call
            3 -> R.string.mode_communication
            else -> R.string.mode_normal
        },
    )

@Composable
private fun SectionTitle(res: Int) {
    Text(stringResource(res), style = MaterialTheme.typography.titleMedium, modifier = Modifier.padding(top = 8.dp))
}

@Composable
private fun Muted(text: String) {
    Text(text, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
}

@Composable
private fun StreamRow(context: Context, stream: Stream, state: UiModel, packages: PackageNames) {
    val device = state.devices.firstOrNull { it.port == stream.devicePort }
    val target =
        device?.let { DeviceLabels.label(context, it) }
            ?: stream.deviceType?.let { "type $it" }
            ?: stringResource(R.string.stream_unknown_device)
    Card(modifier = Modifier.fillMaxWidth()) {
        Column(Modifier.padding(12.dp)) {
            Text(packages.labelForUid(stream.uid) ?: stringResource(R.string.uid_unknown), style = MaterialTheme.typography.titleSmall)
            val kind = stringResource(if (stream.record) R.string.stream_record else R.string.stream_playback)
            Text(
                if (device == null && stream.deviceType == null) kind else "$kind " + stringResource(R.string.stream_to, target),
                style = MaterialTheme.typography.bodyMedium,
            )
        }
    }
}

@Composable
private fun DeviceRow(context: Context, device: AudioDevice) {
    val arrow = if (device.direction == Direction.Input) "↤" else "↦"
    val label = DeviceLabels.label(context, device)
    Muted("$arrow " + if (device.masked) stringResource(R.string.device_hidden_note, label) else label)
}

/** Resolves the Linux uid of an audio stream to the label of the app that owns it. */
private class PackageNames(context: Context) {
    private val packageManager = context.packageManager

    fun labelForUid(uid: Int): String? {
        if (uid < 0) return null
        val packages = packageManager.getPackagesForUid(uid) ?: return null
        return packages.firstNotNullOfOrNull { name ->
            runCatching { packageManager.getApplicationInfo(name, 0).loadLabel(packageManager).toString() }.getOrNull()
        } ?: packages.firstOrNull()
    }
}
