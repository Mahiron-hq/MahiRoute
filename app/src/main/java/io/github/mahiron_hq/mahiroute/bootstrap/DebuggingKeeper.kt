// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

package io.github.mahiron_hq.mahiroute.bootstrap

/**
 * Switches wireless debugging back on after Android's "allow on this network?" prompt.
 *
 * When the app turns the setting on for a network Android does not trust yet, Android shows the
 * prompt and puts the setting back to off. The user's "OK" then starts the debugging server for a
 * moment and shuts it down again, because the setting is off. Only a later write of the setting
 * sticks. Without this, the first attempt ended without a helper and a second tap on "Start the
 * helper" was needed.
 *
 * [tick] is called regularly while the app waits. It writes the setting again when the prompt has
 * probably been closed (the app is back in the foreground), or, as a fallback for a start that
 * runs in the background, after a long pause. Writing while the prompt is still open only shows it
 * again, so the writes are spaced out and limited.
 */
internal class DebuggingKeeper(
    private val clock: () -> Long,
    private val appInForeground: () -> Boolean,
    private val isOn: () -> Boolean,
    private val turnOn: () -> Unit,
) {
    private var lastWriteAt = clock()
    private var writes = 0

    fun tick() {
        if (writes >= MAX_WRITES || isOn()) return
        val sinceLastWrite = clock() - lastWriteAt
        val promptProbablyClosed = appInForeground() && sinceLastWrite >= MIN_GAP_MS
        if (promptProbablyClosed || sinceLastWrite >= BACKGROUND_GAP_MS) {
            lastWriteAt = clock()
            writes += 1
            turnOn()
        }
    }

    companion object {
        /** The shortest time between two writes. */
        const val MIN_GAP_MS = 2_000L

        /** Gap after which the setting is written again even if the app never came back to the foreground. */
        const val BACKGROUND_GAP_MS = 12_000L

        /** Upper limit of repeated writes per start. */
        const val MAX_WRITES = 10
    }
}
