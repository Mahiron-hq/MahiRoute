// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

package io.github.mahiron_hq.mahiroute.ui

import android.content.Context
import android.os.Build
import androidx.annotation.StringRes
import io.github.mahiron_hq.mahiroute.R
import io.github.mahiron_hq.mahiroute.core.AudioDevice
import io.github.mahiron_hq.mahiroute.core.DeviceKey
import io.github.mahiron_hq.mahiroute.core.Direction

/** Human-readable names for devices, built from their kind, position and Bluetooth/USB name. */
object DeviceLabels {
    private val bluetoothKinds = setOf("bluetooth_sco", "bluetooth_a2dp", "ble_headset", "ble_speaker", "hearing_aid")
    private val usbKinds = setOf("usb_headset", "usb_device", "usb_accessory")

    @StringRes
    private fun kindRes(kind: String): Int =
        when (kind) {
            "builtin_earpiece" -> R.string.kind_builtin_earpiece
            "builtin_speaker" -> R.string.kind_builtin_speaker
            "builtin_speaker_safe" -> R.string.kind_builtin_speaker_safe
            "builtin_mic" -> R.string.kind_builtin_mic
            "wired_headset" -> R.string.kind_wired_headset
            "wired_headphones" -> R.string.kind_wired_headphones
            "line_analog" -> R.string.kind_line_analog
            "line_digital" -> R.string.kind_line_digital
            "usb_headset" -> R.string.kind_usb_headset
            "usb_device" -> R.string.kind_usb_device
            "usb_accessory" -> R.string.kind_usb_accessory
            "bluetooth_sco" -> R.string.kind_bluetooth_sco
            "bluetooth_a2dp" -> R.string.kind_bluetooth_a2dp
            "ble_headset" -> R.string.kind_ble_headset
            "ble_speaker" -> R.string.kind_ble_speaker
            "ble_broadcast" -> R.string.kind_ble_broadcast
            "hearing_aid" -> R.string.kind_hearing_aid
            "hdmi" -> R.string.kind_hdmi
            "dock" -> R.string.kind_dock
            "aux_line" -> R.string.kind_aux_line
            else -> R.string.kind_virtual
        }

    fun isBluetooth(kind: String): Boolean = kind in bluetoothKinds

    fun label(context: Context, device: AudioDevice): String = compose(context, device.kind, device.address, device.name)

    /** Name for a remembered choice whose device is not connected right now. */
    fun label(context: Context, key: DeviceKey): String =
        // A USB key carries the product name where other kinds carry the hardware address.
        compose(context, key.kind, key.address, name = if (key.kind in usbKinds) key.address else "")

    fun directionNoun(context: Context, direction: Direction): String =
        context.getString(if (direction == Direction.Input) R.string.direction_input else R.string.direction_output)

    private fun compose(context: Context, kind: String, address: String, name: String): String {
        val base = context.getString(kindRes(kind))
        val detail = detail(context, kind, address, name) ?: return base
        return context.getString(R.string.device_with_detail, base, detail)
    }

    private fun detail(context: Context, kind: String, address: String, name: String): String? =
        when {
            kind == "builtin_mic" -> positionLabel(context, address)
            kind in bluetoothKinds || kind in usbKinds -> name.takeIf { it.isNotBlank() && it != Build.MODEL }
            else -> null
        }

    private fun positionLabel(context: Context, address: String): String? =
        when (address) {
            "" -> null
            "bottom" -> context.getString(R.string.position_bottom)
            "back" -> context.getString(R.string.position_back)
            "front" -> context.getString(R.string.position_front)
            "top" -> context.getString(R.string.position_top)
            else -> address
        }
}
