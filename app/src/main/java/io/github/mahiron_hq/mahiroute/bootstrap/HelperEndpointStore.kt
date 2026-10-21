// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

package io.github.mahiron_hq.mahiroute.bootstrap

import android.content.Context
import androidx.core.content.edit

/** Remembers where the running helper listens so a restarted app can reconnect to it. */
object HelperEndpointStore {
    private const val PREFS = "helper"
    private const val KEY_PORT = "port"
    private const val KEY_TOKEN = "token"

    data class Endpoint(val port: Int, val token: String)

    fun save(context: Context, port: Int, token: String) {
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit {
            putInt(KEY_PORT, port)
            putString(KEY_TOKEN, token)
        }
    }

    fun load(context: Context): Endpoint? {
        val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        val port = prefs.getInt(KEY_PORT, 0)
        val token = prefs.getString(KEY_TOKEN, null)
        return if (port > 0 && token != null) Endpoint(port, token) else null
    }
}
