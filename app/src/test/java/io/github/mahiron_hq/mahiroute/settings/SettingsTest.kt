// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

package io.github.mahiron_hq.mahiroute.settings

import io.github.mahiron_hq.mahiroute.core.AudioDevice
import io.github.mahiron_hq.mahiroute.core.Direction
import io.github.mahiron_hq.mahiroute.core.Link
import io.github.mahiron_hq.mahiroute.core.ReportText
import io.github.mahiron_hq.mahiroute.core.UiModel
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class SettingsTest {
    @Test
    fun unknownStoredValuesFallBackToTheSystem() {
        assertEquals(ThemeMode.System, ThemeMode.fromStored(null))
        assertEquals(ThemeMode.System, ThemeMode.fromStored("purple"))
        assertEquals(ThemeMode.Dark, ThemeMode.fromStored("dark"))
        assertEquals(LanguageChoice.System, LanguageChoice.fromStored("de"))
        assertEquals(LanguageChoice.Russian, LanguageChoice.fromStored("ru"))
    }

    @Test
    fun theSystemLanguageHasNoFixedTag() {
        assertNull(LanguageChoice.System.tag)
        assertEquals("en", LanguageChoice.English.tag)
    }

    @Test
    fun aNewerNoticeNeedsANewAcceptance() {
        assertTrue(needsConsent(acceptedVersion = 0, currentVersion = 1))
        assertTrue(needsConsent(acceptedVersion = 1, currentVersion = 2))
        assertFalse(needsConsent(acceptedVersion = 2, currentVersion = 2))
    }

    @Test
    fun theReportHidesAddressesAndNames() {
        val bluetooth =
            AudioDevice(7, Direction.Output, "bluetooth_a2dp", 8, "AA:BB:CC:DD:EE:01", "My private buds", masked = true)
        val state = UiModel.EMPTY.copy(link = Link.Ready, devices = listOf(bluetooth), warnings = listOf("lost AA:BB:CC:DD:EE:01"))
        val report = ReportText.build(state, "1.0.2 (3)", "13", "Test Phone")
        assertTrue(report.contains("bluetooth_a2dp (not offered)"))
        assertFalse(report.contains("AA:BB"))
        assertFalse(report.contains("My private buds"))
        assertTrue(report.contains("xx:xx:xx:xx:xx:xx"))
    }
}
