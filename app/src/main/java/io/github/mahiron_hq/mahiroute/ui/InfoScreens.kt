// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

package io.github.mahiron_hq.mahiroute.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import io.github.mahiron_hq.mahiroute.R

private const val REPOSITORY = "https://github.com/Mahiron-hq/MahiRoute"

private val policySections =
    listOf(
        R.string.policy_h1 to R.string.policy_b1,
        R.string.policy_h2 to R.string.policy_b2,
        R.string.policy_h3 to R.string.policy_b3,
        R.string.policy_h4 to R.string.policy_b4,
        R.string.policy_h5 to R.string.policy_b5,
        R.string.policy_h6 to R.string.policy_b6,
        R.string.policy_h7 to R.string.policy_b7,
    )

@Composable
private fun PolicyText(modifier: Modifier = Modifier) {
    Column(modifier, verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text(stringResource(R.string.policy_updated), style = MaterialTheme.typography.labelMedium)
        policySections.forEach { (heading, body) ->
            Text(stringResource(heading), style = MaterialTheme.typography.titleMedium)
            Text(stringResource(body), style = MaterialTheme.typography.bodyMedium)
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun InfoScaffold(titleRes: Int, onBack: (() -> Unit)?, content: @Composable ColumnScope.() -> Unit) {
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(titleRes)) },
                navigationIcon = {
                    if (onBack != null) {
                        IconButton(onClick = onBack) {
                            Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = stringResource(R.string.back))
                        }
                    }
                },
            )
        },
    ) { padding ->
        Column(Modifier.padding(padding).fillMaxSize(), content = content)
    }
}

/** The privacy notice on its own, reached from settings and from the about screen. */
@Composable
fun PolicyScreen(onBack: () -> Unit) {
    InfoScaffold(R.string.policy_title, onBack) {
        PolicyText(Modifier.verticalScroll(rememberScrollState()).padding(16.dp))
    }
}

/** Shown before anything else until the current privacy notice is accepted. */
@Composable
fun ConsentScreen(updated: Boolean, onAccept: () -> Unit, onDecline: () -> Unit) {
    InfoScaffold(R.string.consent_title, onBack = null) {
        Column(
            Modifier.weight(1f).verticalScroll(rememberScrollState()).padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Text(
                stringResource(if (updated) R.string.consent_updated else R.string.consent_intro),
                style = MaterialTheme.typography.bodyLarge,
            )
            PolicyText()
        }
        Row(Modifier.fillMaxWidth().padding(16.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            OutlinedButton(onClick = onDecline, modifier = Modifier.weight(1f)) {
                Text(stringResource(R.string.consent_decline))
            }
            Button(onClick = onAccept, modifier = Modifier.weight(1f)) { Text(stringResource(R.string.consent_accept)) }
        }
    }
}

/** What the about screen needs from the outside world. */
class AboutInfo(
    val versionName: String,
    val versionCode: Int,
    val onOpenLink: (String) -> Unit,
    val onOpenPolicy: () -> Unit,
)

@Composable
fun AboutScreen(info: AboutInfo, onBack: () -> Unit) {
    InfoScaffold(R.string.about_title, onBack) {
        Column(
            Modifier.verticalScroll(rememberScrollState()).padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Text(stringResource(R.string.app_name), style = MaterialTheme.typography.headlineMedium)
            Text(
                stringResource(R.string.about_version, info.versionName, info.versionCode),
                style = MaterialTheme.typography.bodyLarge,
            )
            Text(stringResource(R.string.about_license), style = MaterialTheme.typography.bodyMedium)
            Text(stringResource(R.string.about_author), style = MaterialTheme.typography.bodyMedium)
            TextButton(onClick = info.onOpenPolicy) { Text(stringResource(R.string.privacy_policy)) }
            TextButton(onClick = { info.onOpenLink(REPOSITORY) }) { Text(stringResource(R.string.about_source)) }
            TextButton(onClick = { info.onOpenLink("$REPOSITORY/releases") }) {
                Text(stringResource(R.string.about_releases))
            }
            TextButton(onClick = { info.onOpenLink("$REPOSITORY/issues") }) {
                Text(stringResource(R.string.about_issues))
            }
            Text(stringResource(R.string.about_third_party), style = MaterialTheme.typography.titleMedium)
            Text(stringResource(R.string.about_third_party_text), style = MaterialTheme.typography.bodyMedium)
        }
    }
}
