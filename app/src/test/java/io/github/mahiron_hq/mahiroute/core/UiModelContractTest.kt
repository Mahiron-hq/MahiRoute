// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

package io.github.mahiron_hq.mahiroute.core

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** Parses the state document the Rust engine pins in its own contract test. */
class UiModelContractTest {
    private val model: UiModel =
        UiModel.parse(
            checkNotNull(javaClass.classLoader?.getResource("ui_state_contract.json")) { "golden file missing" }.readText(),
        )

    @Test
    fun linkAndFirmware() {
        assertEquals(Link.Ready, model.link)
        assertEquals(33, model.sdk)
        assertEquals(3, model.mode)
        assertTrue(model.enabled)
    }

    @Test
    fun selectionIsReadAsDeviceKeys() {
        assertEquals(DeviceKey(Direction.Input, "builtin_mic", "bottom"), model.input)
        assertEquals(DeviceKey(Direction.Output, "bluetooth_a2dp", "AA:BB:CC:DD:EE:FF"), model.output)
    }

    @Test
    fun deviceKeysSerializeBackToTheEngineFormat() {
        val json = org.json.JSONObject(checkNotNull(model.input).toJson())
        assertEquals("input", json.getString("direction"))
        assertEquals("builtin_mic", json.getString("kind"))
        assertEquals("bottom", json.getString("address"))
    }

    @Test
    fun devicesIncludeMaskedOnesAndKeepTheirDirection() {
        assertEquals(3, model.devicesFor(Direction.Input).size)
        assertEquals(2, model.devicesFor(Direction.Output).size)
        val hidden = model.devicesFor(Direction.Input).single { it.masked }
        assertEquals("wired_headset", hidden.kind)
        assertEquals(0x8000_0000L, hidden.port)
    }

    @Test
    fun chosenDeviceIsFoundWhenConnectedAndAbsentWhenNot() {
        assertEquals("bottom", model.chosenDevice(Direction.Input)?.address)
        assertNull(model.chosenDevice(Direction.Output))
        assertEquals(setOf(Direction.Output), model.missing)
    }

    @Test
    fun routingCapabilitiesAndStreams() {
        assertEquals(Routing.Applying, model.routing)
        assertEquals(Support.Ignored, model.capturePreset)
        assertEquals(Support.Honored, model.outputStrategy)
        assertEquals(2, model.streams.size)
        val recording = model.streams.first()
        assertTrue(recording.record)
        assertEquals(10_169, recording.uid)
        assertEquals(16L, recording.devicePort)
        assertNull(model.streams.last().devicePort)
        assertFalse(model.streams.last().record)
    }

    @Test
    fun closedWarningsAreSeparatedFromMissingDevices() {
        assertEquals(setOf(Direction.Output), model.dismissedMissing)
        assertTrue(model.warnedMissing.isEmpty())
        val reopened = model.copy(dismissedMissing = emptySet())
        assertEquals(listOf(Direction.Output), reopened.warnedMissing)
    }

    @Test
    fun usbDevicesAreIdentifiedByNameBecauseTheirAddressIsRenumbered() {
        val dac = AudioDevice(5, Direction.Output, "usb_device", 11, "card=1;device=0", "USB-C DAC", masked = false)
        val moved = dac.copy(port = 9, address = "card=3;device=0")
        assertEquals(dac.key, moved.key)
        assertEquals("USB-C DAC", dac.key.address)
        val other = dac.copy(name = "Gaming Headset")
        assertTrue(dac.key != other.key)
        val bluetooth = AudioDevice(6, Direction.Output, "bluetooth_a2dp", 8, "AA:BB", "Buds", masked = false)
        assertEquals("AA:BB", bluetooth.key.address)
    }

    @Test
    fun warningsArrive() {
        assertEquals(listOf("unexpected helper response 99"), model.warnings)
        assertNotNull(model.devices)
    }
}
