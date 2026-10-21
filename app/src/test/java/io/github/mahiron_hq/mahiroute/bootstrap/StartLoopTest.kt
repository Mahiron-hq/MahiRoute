// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

package io.github.mahiron_hq.mahiroute.bootstrap

import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** Runs [StartLoop] on a virtual clock: waiting only advances a counter, no real time passes. */
class StartLoopTest {
    private var now = 0L
    private val steps = mutableListOf<Step>()
    private var startCalls = 0
    private var keepCalls = 0
    private val excludedSeen = mutableListOf<Set<Int>>()

    private fun loop(servicePortAppearsAt: Long?, answers: List<Started>): StartLoop {
        val queue = answers.toMutableList()
        return StartLoop(
            clock = { now },
            pause = { now += it },
            keepEnabled = { keepCalls += 1 },
            findPort = { timeout, excluded ->
                excludedSeen += excluded.toSet()
                val appears = servicePortAppearsAt
                if (appears != null && now + timeout >= appears) {
                    now = maxOf(now, appears)
                    PORT
                } else {
                    now += timeout
                    null
                }
            },
            start = {
                startCalls += 1
                queue.removeAt(0)
            },
            onStep = { steps += it },
        )
    }

    @Test
    fun anAnswerThatTakesTwentySecondsStillStartsTheHelperOnTheFirstTry() =
        runBlocking {
            val result = loop(servicePortAppearsAt = 20_000, answers = listOf(READY)).run()
            assertEquals(READY, result)
            assertEquals(1, startCalls)
            assertTrue("the user is told to look for Android's prompt", Step.WaitingForConfirmation in steps)
            assertEquals(Step.FindingService, steps.first())
            assertEquals(Step.StartingHelper, steps.last())
        }

    @Test
    fun theSettingIsCheckedBeforeEveryDiscoveryAttempt() =
        runBlocking {
            loop(servicePortAppearsAt = 20_000, answers = listOf(READY)).run()
            // Four slices of four seconds pass before the service shows up during the fifth attempt.
            assertEquals(5, keepCalls)
        }

    @Test
    fun aServiceThatIsThereAtOnceNeverShowsThePromptHint() =
        runBlocking {
            val result = loop(servicePortAppearsAt = 0, answers = listOf(READY)).run()
            assertEquals(READY, result)
            assertTrue(Step.WaitingForConfirmation !in steps)
            assertEquals(0L, now)
        }

    @Test
    fun aRefusedConnectionWhileThePromptIsOpenIsRetried() =
        runBlocking {
            val refused = Started.Failed(Problem.Unreachable, "connection refused")
            val result = loop(servicePortAppearsAt = 0, answers = listOf(refused, refused, READY)).run()
            assertEquals(READY, result)
            assertEquals(3, startCalls)
            assertEquals(2 * StartLoop.RETRY_PAUSE_MS, now)
        }

    @Test
    fun aPortThatRefusedAConnectionIsNotTriedAgain() =
        runBlocking {
            val refused = Started.Failed(Problem.Unreachable, "connection refused")
            loop(servicePortAppearsAt = 0, answers = listOf(refused, READY)).run()
            assertEquals(listOf(emptySet(), setOf(PORT)), excludedSeen)
        }

    @Test
    fun giveUpAfterTheWindowWhenNothingEverAppears() =
        runBlocking {
            val result = loop(servicePortAppearsAt = null, answers = emptyList()).run()
            assertEquals(Started.Failed(Problem.ServiceNotFound, null), result)
            assertEquals(0, startCalls)
            assertEquals(StartLoop.WINDOW_MS, now)
        }

    @Test
    fun aRefusalThatNeverEndsIsReportedAsUnreachableAfterTheWindow() =
        runBlocking {
            val refused = Started.Failed(Problem.Unreachable, "connection refused")
            val result = loop(servicePortAppearsAt = 0, answers = List(100) { refused }).run()
            assertEquals(refused, result)
            assertTrue(now >= StartLoop.WINDOW_MS)
        }

    @Test
    fun pairingIsNeverRetriedBlindly() =
        runBlocking {
            val result = loop(servicePortAppearsAt = 0, answers = listOf(Started.NotPaired)).run()
            assertEquals(Started.NotPaired, result)
            assertEquals(1, startCalls)
        }

    @Test
    fun otherFailuresEndTheLoopAtOnce() =
        runBlocking {
            val broken = Started.Failed(Problem.Other, "boom")
            val result = loop(servicePortAppearsAt = 0, answers = listOf(broken)).run()
            assertEquals(broken, result)
            assertEquals(1, startCalls)
        }

    private companion object {
        const val PORT = 43_000
        val READY = Started.Ready(PORT, "token")
    }
}
