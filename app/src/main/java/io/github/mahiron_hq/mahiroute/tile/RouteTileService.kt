// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

package io.github.mahiron_hq.mahiroute.tile

import android.content.Context
import android.service.quicksettings.Tile
import android.service.quicksettings.TileService
import io.github.mahiron_hq.mahiroute.R
import io.github.mahiron_hq.mahiroute.core.EngineHost
import io.github.mahiron_hq.mahiroute.core.Link
import io.github.mahiron_hq.mahiroute.core.Routing
import io.github.mahiron_hq.mahiroute.core.UiModel
import io.github.mahiron_hq.mahiroute.settings.LocaleSupport
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.collect
import kotlinx.coroutines.launch

/** Quick Settings tile that switches the routing on and off without opening the app. */
class RouteTileService : TileService() {
    override fun attachBaseContext(newBase: Context) {
        super.attachBaseContext(LocaleSupport.wrap(newBase))
    }

    private val scope = CoroutineScope(Dispatchers.Main.immediate)
    private var job: Job? = null

    override fun onStartListening() {
        super.onStartListening()
        EngineHost.start(applicationContext)
        job = scope.launch { EngineHost.state.collect(::render) }
    }

    override fun onStopListening() {
        job?.cancel()
        super.onStopListening()
    }

    override fun onClick() {
        super.onClick()
        EngineHost.setEnabled(!EngineHost.state.value.enabled)
    }

    private fun render(state: UiModel) {
        val tile = qsTile ?: return
        tile.label = getString(R.string.tile_label)
        tile.state = if (state.enabled) Tile.STATE_ACTIVE else Tile.STATE_INACTIVE
        tile.subtitle =
            when {
                !state.enabled -> getString(R.string.switch_enabled_hint_off)
                state.link !is Link.Ready && (state.input != null || state.output != null) -> getString(R.string.helper_title)
                state.routing is Routing.Verified -> getString(R.string.routing_verified)
                else -> null
            }
        tile.updateTile()
    }
}
