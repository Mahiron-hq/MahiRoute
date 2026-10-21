// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

package io.github.mahiron_hq.mahiroute

import android.app.Application
import io.github.mahiron_hq.mahiroute.bootstrap.AppForeground
import io.github.mahiron_hq.mahiroute.bootstrap.AutoStarter
import io.github.mahiron_hq.mahiroute.bootstrap.HelperEndpointStore
import io.github.mahiron_hq.mahiroute.core.EngineHost
import io.github.mahiron_hq.mahiroute.service.RoutingService
import io.github.mahiron_hq.mahiroute.startup.StartupPolicy
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch

class MahiRouteApp : Application() {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)

    override fun onCreate() {
        super.onCreate()
        StartupPolicy.load(this)
        AppForeground.register(this)
        EngineHost.start(this)
        // A helper that outlived the app process is still listening; pick it up again.
        HelperEndpointStore.load(this)?.let { EngineHost.connect(it.port, it.token) }
        scope.launch {
            EngineHost.state.map(RoutingService::wanted).distinctUntilChanged().collect { wanted ->
                if (wanted) RoutingService.start(this@MahiRouteApp) else RoutingService.stop(this@MahiRouteApp)
            }
        }
        AutoStarter(this, scope).start()
    }
}
