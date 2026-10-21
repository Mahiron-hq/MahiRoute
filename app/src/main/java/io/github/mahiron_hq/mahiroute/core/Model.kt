// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

package io.github.mahiron_hq.mahiroute.core

import org.json.JSONArray
import org.json.JSONObject

enum class Direction(val wire: String) {
    Input("input"),
    Output("output");

    companion object {
        fun fromWire(value: String): Direction = entries.firstOrNull { it.wire == value } ?: Output
    }
}

/** USB addresses encode a card number the kernel may renumber, so USB devices are told apart by name. */
internal fun hasStableAddress(kind: String): Boolean = kind != "usb_headset" && kind != "usb_device" && kind != "usb_accessory"

/** Reconnect-proof identity of a device; the engine persists it. [address] is the product name for USB kinds. */
data class DeviceKey(val direction: Direction, val kind: String, val address: String) {
    fun toJson(): String =
        JSONObject()
            .put("direction", direction.wire)
            .put("kind", kind)
            .put("address", address)
            .toString()
}

data class AudioDevice(
    val port: Long,
    val direction: Direction,
    val kind: String,
    val androidType: Int,
    val address: String,
    val name: String,
    val masked: Boolean,
) {
    /** What tells this device apart from another of the same kind after a reconnect. */
    val identity: String get() = if (hasStableAddress(kind)) address else name

    val key: DeviceKey get() = DeviceKey(direction, kind, identity)
}

sealed interface Link {
    data object Disconnected : Link

    data object Connecting : Link

    data object Ready : Link

    data class Failed(val reason: String) : Link
}

sealed interface Routing {
    data object Inactive : Routing

    data object Applying : Routing

    data object Verifying : Routing

    data object Verified : Routing

    data object Unverified : Routing

    data class Failed(val direction: Direction) : Routing
}

data class Stream(
    val record: Boolean,
    val uid: Int,
    val source: Int,
    val devicePort: Long?,
    val deviceType: Int?,
)

enum class Support {
    Unknown,
    Honored,
    Ignored;

    companion object {
        fun fromWire(value: String): Support = entries.firstOrNull { it.name.equals(value, ignoreCase = true) } ?: Unknown
    }
}

/** What the engine knows about the audio system, as the interface renders it. */
data class UiModel(
    val link: Link,
    val sdk: Int?,
    val enabled: Boolean,
    val input: DeviceKey?,
    val output: DeviceKey?,
    val devices: List<AudioDevice>,
    val missing: Set<Direction>,
    /** The part of [missing] whose warning the user has closed. */
    val dismissedMissing: Set<Direction>,
    val routing: Routing,
    val capturePreset: Support,
    val outputStrategy: Support,
    val streams: List<Stream>,
    val mode: Int,
    val warnings: List<String>,
) {
    fun choice(direction: Direction): DeviceKey? = if (direction == Direction.Input) input else output

    fun devicesFor(direction: Direction): List<AudioDevice> = devices.filter { it.direction == direction }

    /** The device the user chose, if it is currently connected. */
    fun chosenDevice(direction: Direction): AudioDevice? {
        val key = choice(direction) ?: return null
        return devicesFor(direction).firstOrNull { it.key == key }
    }

    /** Directions whose chosen device is gone and whose warning is still to be shown. */
    val warnedMissing: List<Direction> get() = Direction.entries.filter { it in missing && it !in dismissedMissing }

    companion object {
        val EMPTY =
            UiModel(
                link = Link.Disconnected,
                sdk = null,
                enabled = true,
                input = null,
                output = null,
                devices = emptyList(),
                missing = emptySet(),
                dismissedMissing = emptySet(),
                routing = Routing.Inactive,
                capturePreset = Support.Unknown,
                outputStrategy = Support.Unknown,
                streams = emptyList(),
                mode = 0,
                warnings = emptyList(),
            )

        fun parse(json: String): UiModel {
            val root = JSONObject(json)
            val selection = root.getJSONObject("selection")
            val capabilities = root.getJSONObject("capabilities")
            return UiModel(
                link = parseLink(root.getJSONObject("link")),
                sdk = if (root.isNull("sdk")) null else root.getInt("sdk"),
                enabled = root.getBoolean("enabled"),
                input = parseChoice(selection.getJSONObject("input")),
                output = parseChoice(selection.getJSONObject("output")),
                devices = root.getJSONArray("devices").mapObjects(::parseDevice),
                missing = root.getJSONArray("missing").mapStrings().map(Direction::fromWire).toSet(),
                dismissedMissing = root.getJSONArray("dismissed_missing").mapStrings().map(Direction::fromWire).toSet(),
                routing = parseRouting(root.getJSONObject("routing")),
                capturePreset = Support.fromWire(capabilities.getString("capture_preset")),
                outputStrategy = Support.fromWire(capabilities.getString("output_strategy")),
                streams = root.getJSONArray("streams").mapObjects(::parseStream),
                mode = root.getInt("mode"),
                warnings = root.getJSONArray("warnings").mapStrings(),
            )
        }

        private fun parseLink(json: JSONObject): Link =
            when (json.getString("state")) {
                "ready" -> Link.Ready
                "connecting" -> Link.Connecting
                "failed" -> Link.Failed(json.optString("reason"))
                else -> Link.Disconnected
            }

        private fun parseRouting(json: JSONObject): Routing =
            when (json.getString("state")) {
                "applying" -> Routing.Applying
                "verifying" -> Routing.Verifying
                "verified" -> Routing.Verified
                "unverified" -> Routing.Unverified
                "failed" -> Routing.Failed(Direction.fromWire(json.getString("direction")))
                else -> Routing.Inactive
            }

        private fun parseChoice(json: JSONObject): DeviceKey? {
            if (json.getString("kind") != "device") return null
            val device = json.getJSONObject("device")
            return DeviceKey(
                Direction.fromWire(device.getString("direction")),
                device.getString("kind"),
                device.getString("address"),
            )
        }

        private fun parseDevice(json: JSONObject) =
            AudioDevice(
                port = json.getLong("port"),
                direction = Direction.fromWire(json.getString("direction")),
                kind = json.getString("kind"),
                androidType = json.getInt("android_type"),
                address = json.getString("address"),
                name = json.getString("name"),
                masked = json.getBoolean("masked"),
            )

        private fun parseStream(json: JSONObject) =
            Stream(
                record = json.getString("kind") == "record",
                uid = json.getInt("uid"),
                source = json.getInt("source"),
                devicePort = if (json.isNull("device_port")) null else json.getLong("device_port"),
                deviceType = if (json.isNull("device_type")) null else json.getInt("device_type"),
            )
    }
}

private inline fun <T> JSONArray.mapObjects(transform: (JSONObject) -> T): List<T> =
    List(length()) { transform(getJSONObject(it)) }

private fun JSONArray.mapStrings(): List<String> = List(length()) { getString(it) }
