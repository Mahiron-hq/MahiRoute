// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

package io.github.mahiron_hq.mahiroute.bootstrap

import org.junit.Assert.assertEquals
import org.junit.Test

class DebuggingKeeperTest {
    private var now = 0L
    private var inForeground = false
    private var on = false
    private var writes = 0

    private fun keeper() =
        DebuggingKeeper(
            clock = { now },
            appInForeground = { inForeground },
            isOn = { on },
            turnOn = { writes += 1 },
        )

    @Test
    fun nothingIsWrittenWhileThePromptCoversTheApp() {
        val keeper = keeper()
        now = 8_000
        keeper.tick()
        assertEquals(0, writes)
    }

    @Test
    fun theSettingIsWrittenAgainWhenTheAppIsBackAfterThePrompt() {
        val keeper = keeper()
        now = 9_000
        inForeground = true
        keeper.tick()
        assertEquals(1, writes)
    }

    @Test
    fun aWriteRightAfterTheLastOneWaits() {
        val keeper = keeper()
        inForeground = true
        now = DebuggingKeeper.MIN_GAP_MS - 1
        keeper.tick()
        assertEquals(0, writes)
        now = DebuggingKeeper.MIN_GAP_MS
        keeper.tick()
        assertEquals(1, writes)
    }

    @Test
    fun aStartInTheBackgroundStillRetriesAfterALongPause() {
        val keeper = keeper()
        now = DebuggingKeeper.BACKGROUND_GAP_MS - 1
        keeper.tick()
        assertEquals(0, writes)
        now = DebuggingKeeper.BACKGROUND_GAP_MS
        keeper.tick()
        assertEquals(1, writes)
    }

    @Test
    fun aSettingThatIsAlreadyOnIsLeftAlone() {
        val keeper = keeper()
        on = true
        inForeground = true
        now = 60_000
        keeper.tick()
        assertEquals(0, writes)
    }

    @Test
    fun theNumberOfWritesIsLimited() {
        val keeper = keeper()
        inForeground = true
        repeat(100) {
            now += DebuggingKeeper.MIN_GAP_MS
            keeper.tick()
        }
        assertEquals(DebuggingKeeper.MAX_WRITES, writes)
    }
}
