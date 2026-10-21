// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

package io.github.mahiron_hq.mahiroute.core

/** The plain-text report a user can attach to a bug report. */
object ReportText {
    private val MAC_ADDRESS = Regex("[0-9A-Fa-f]{2}(:[0-9A-Fa-f]{2}){5}")

    /**
     * Describes the app and the audio state without anything personal: hardware addresses, device
     * names and the apps that play or record sound are left out.
     */
    fun build(state: UiModel, appVersion: String, androidRelease: String, phone: String): String =
        buildString {
            appendLine("MahiRoute $appVersion")
            appendLine("Android $androidRelease (API ${state.sdk ?: "unknown"}), $phone")
            appendLine("Helper: ${link(state.link)}")
            appendLine("Routing enabled: ${state.enabled}, state: ${state.routing::class.simpleName}")
            appendLine("Firmware: capture preference ${state.capturePreset}, output preference ${state.outputStrategy}")
            appendLine("Audio mode: ${state.mode}")
            appendLine("Input chosen: ${state.input?.kind ?: "system default"}, output chosen: ${state.output?.kind ?: "system default"}")
            appendLine("Missing: ${state.missing.joinToString { it.wire }.ifEmpty { "none" }}")
            appendLine("Devices:")
            state.devices.forEach { appendLine("  ${it.direction.wire} ${it.kind}${if (it.masked) " (not offered)" else ""}") }
            if (state.warnings.isNotEmpty()) {
                appendLine("Warnings:")
                state.warnings.forEach { appendLine("  ${it.replace(MAC_ADDRESS, "xx:xx:xx:xx:xx:xx")}") }
            }
        }

    private fun link(link: Link): String =
        when (link) {
            Link.Disconnected -> "disconnected"
            Link.Connecting -> "connecting"
            Link.Ready -> "ready"
            is Link.Failed -> "failed (${link.reason})"
        }
}
