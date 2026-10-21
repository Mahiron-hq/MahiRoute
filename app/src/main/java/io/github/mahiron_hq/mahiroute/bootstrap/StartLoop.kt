// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

package io.github.mahiron_hq.mahiroute.bootstrap

/** How one attempt to start the helper through wireless debugging ended. */
internal sealed interface Started {
    data class Ready(val port: Int, val token: String) : Started

    /** The app's key is not trusted yet: the user has to pair first. */
    data object NotPaired : Started

    data class Failed(val problem: Problem, val detail: String?) : Started
}

/**
 * Keeps trying to start the helper for a while instead of giving up at the first miss.
 *
 * Right after wireless debugging is switched on Android may first ask the user to allow it on the
 * current network, and `adbd` only advertises its service (and accepts connections) once that is
 * answered. The user needs time to read and tap, so a short wait made the first attempt fail and
 * only a second tap on "Start the helper" worked. [keepEnabled] is called before every discovery
 * attempt so that the setting can be switched on again once the prompt is answered (see
 * [DebuggingKeeper]).
 *
 * The collaborators are injected so the timing can be tested without a device or a real clock.
 */
internal class StartLoop(
    private val clock: () -> Long,
    private val pause: suspend (millis: Long) -> Unit,
    private val keepEnabled: () -> Unit,
    private val findPort: suspend (timeoutMillis: Long, excludedPorts: Set<Int>) -> Int?,
    private val start: suspend (port: Int) -> Started,
    private val onStep: (Step) -> Unit,
) {
    /** Runs until the helper is started, the user has to act, or [WINDOW_MS] has passed. */
    suspend fun run(): Started {
        val begin = clock()
        var last: Started = Started.Failed(Problem.ServiceNotFound, null)
        // Ports that refused a connection: Android may go on announcing a withdrawn service for a while.
        val refusedPorts = mutableSetOf<Int>()
        while (true) {
            val elapsed = clock() - begin
            if (elapsed >= WINDOW_MS) return last
            onStep(if (elapsed >= CONFIRMATION_HINT_AFTER_MS) Step.WaitingForConfirmation else Step.FindingService)
            keepEnabled()
            val port = findPort(minOf(SLICE_MS, WINDOW_MS - elapsed), refusedPorts)
            if (port == null) {
                last = Started.Failed(Problem.ServiceNotFound, null)
                continue
            }
            onStep(Step.StartingHelper)
            when (val result = start(port)) {
                is Started.Ready, Started.NotPaired -> return result
                is Started.Failed -> {
                    // A refused connection is expected while the permission prompt is still open.
                    if (result.problem != Problem.Unreachable) return result
                    last = result
                    refusedPorts += port
                    pause(RETRY_PAUSE_MS)
                }
            }
        }
    }

    companion object {
        /** Total time allowed for the user to answer Android's prompt. */
        const val WINDOW_MS = 120_000L

        /** Length of one discovery attempt. */
        const val SLICE_MS = 4_000L

        /** Pause before connecting again after a refused connection. */
        const val RETRY_PAUSE_MS = 1_500L

        /** After this long without a service the screen tells the user to look for Android's prompt. */
        const val CONFIRMATION_HINT_AFTER_MS = 4_000L
    }
}
