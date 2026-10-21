// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

package io.github.mahiron_hq.mahiroute.bootstrap

import android.app.Application
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import io.github.mahiron_hq.mahiroute.core.EngineHost
import io.github.mahiron_hq.mahiroute.core.Link
import io.github.mahiron_hq.mahiroute.core.UiModel
import io.github.mahiron_hq.mahiroute.service.RoutingService
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.filter
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * Brings the helper up by itself when the user has a routing selection but the helper is not
 * running, for example after the phone restarted: through wireless debugging while Wi-Fi is
 * available, or by picking up a helper that was started from a computer.
 */
class AutoStarter(private val app: Application, private val scope: CoroutineScope) {
    private val wifi = MutableStateFlow(false)

    fun start() {
        val manager = app.getSystemService(ConnectivityManager::class.java)
        val request = NetworkRequest.Builder().addTransportType(NetworkCapabilities.TRANSPORT_WIFI).build()
        manager.registerNetworkCallback(
            request,
            object : ConnectivityManager.NetworkCallback() {
                override fun onAvailable(network: Network) {
                    wifi.value = true
                }

                override fun onLost(network: Network) {
                    wifi.value = false
                }
            },
        )
        // The tick makes the check repeat, so an attempt that failed for a passing reason, such as
        // Wi-Fi coming up late after a restart, is tried again without the user.
        val ticks = flow {
            var tick = 0
            while (true) {
                emit(tick++)
                delay(RETRY_MS)
            }
        }
        scope.launch {
            combine(EngineHost.state, wifi, ticks) { state, onWifi, tick -> (onWifi && needsHelper(state)) to tick }
                .filter { it.first }
                .distinctUntilChanged()
                .collectLatest {
                    // Give an existing helper a moment to answer before starting a new one.
                    delay(SETTLE_MS)
                    if (needsHelper(EngineHost.state.value) && canStart(HelperBootstrap.state.value)) {
                        HelperBootstrap.start(app)
                    }
                }
        }
        // A helper started from a computer listens on a fixed port; connect the moment it appears.
        scope.launch {
            EngineHost.state.map(::needsHelper).distinctUntilChanged().collectLatest { needs ->
                while (needs) {
                    if (withContext(Dispatchers.IO) { PcStart.helperListening() }) {
                        PcStart.connect(app, withContext(Dispatchers.IO) { PcStart.prepare(app) })
                    }
                    delay(PC_POLL_MS)
                }
            }
        }
    }

    // A failure that is not transient (a refused pairing, no code entered) waits for the user.
    private fun canStart(state: BootstrapState): Boolean =
        state is BootstrapState.Idle || (state is BootstrapState.Failed && state.problem != Problem.Other)

    private fun needsHelper(state: UiModel): Boolean =
        RoutingService.wanted(state) && (state.link is Link.Disconnected || state.link is Link.Failed)

    private companion object {
        const val SETTLE_MS = 3_000L
        const val RETRY_MS = 30_000L
        const val PC_POLL_MS = 4_000L
    }
}
