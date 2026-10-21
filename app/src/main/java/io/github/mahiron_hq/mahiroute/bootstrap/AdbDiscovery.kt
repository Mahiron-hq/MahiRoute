// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

package io.github.mahiron_hq.mahiroute.bootstrap

import android.content.Context
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import java.util.concurrent.atomic.AtomicBoolean
import kotlin.coroutines.resume
import kotlinx.coroutines.channels.awaitClose
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.callbackFlow
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withTimeoutOrNull

/** Finds the ports `adbd` advertises on the local network for wireless debugging. */
class AdbDiscovery(context: Context) {
    private val manager = context.getSystemService(NsdManager::class.java)

    /**
     * Port of the first service of [serviceType] that shows up within [timeoutMillis] and is not in
     * [excluded], if any. Android keeps announcing a service for a short while after `adbd` has
     * withdrawn it, so ports that already refused a connection must be skipped.
     */
    suspend fun findPort(serviceType: String, timeoutMillis: Long, excluded: Set<Int> = emptySet()): Int? =
        withTimeoutOrNull(timeoutMillis) {
            suspendCancellableCoroutine { continuation ->
                val lock = Any()
                val pending = ArrayDeque<NsdServiceInfo>()
                var resolving = false
                lateinit var listener: NsdManager.DiscoveryListener

                // NSD rejects concurrent resolves, so the services found are resolved one after another.
                fun pump() {
                    val next =
                        synchronized(lock) {
                            if (resolving || !continuation.isActive) {
                                null
                            } else {
                                pending.removeFirstOrNull()?.also { resolving = true }
                            }
                        } ?: return
                    resolve(next) { port ->
                        synchronized(lock) { resolving = false }
                        if (port != null && port !in excluded && continuation.isActive) {
                            runCatching { manager.stopServiceDiscovery(listener) }
                            continuation.resume(port)
                        } else {
                            pump()
                        }
                    }
                }

                listener =
                    object : NsdManager.DiscoveryListener {
                        override fun onDiscoveryStarted(regType: String) = Unit

                        override fun onServiceFound(serviceInfo: NsdServiceInfo) {
                            synchronized(lock) { pending.addLast(serviceInfo) }
                            pump()
                        }

                        override fun onServiceLost(serviceInfo: NsdServiceInfo) = Unit

                        override fun onDiscoveryStopped(serviceType: String) = Unit

                        override fun onStartDiscoveryFailed(serviceType: String, errorCode: Int) {
                            if (continuation.isActive) continuation.resume(null)
                        }

                        override fun onStopDiscoveryFailed(serviceType: String, errorCode: Int) = Unit
                    }
                manager.discoverServices(serviceType, NsdManager.PROTOCOL_DNS_SD, listener)
                continuation.invokeOnCancellation { runCatching { manager.stopServiceDiscovery(listener) } }
            }
        }

    /** Emits the port of the matching service while it is advertised and null once it disappears. */
    fun watch(serviceType: String): Flow<Int?> =
        callbackFlow {
            val resolving = AtomicBoolean(false)
            val listener =
                object : NsdManager.DiscoveryListener {
                    override fun onDiscoveryStarted(regType: String) = Unit

                    override fun onServiceFound(serviceInfo: NsdServiceInfo) {
                        if (!resolving.compareAndSet(false, true)) return
                        resolve(serviceInfo) { port ->
                            resolving.set(false)
                            if (port != null) trySend(port)
                        }
                    }

                    override fun onServiceLost(serviceInfo: NsdServiceInfo) {
                        trySend(null)
                    }

                    override fun onDiscoveryStopped(serviceType: String) = Unit

                    override fun onStartDiscoveryFailed(serviceType: String, errorCode: Int) {
                        close()
                    }

                    override fun onStopDiscoveryFailed(serviceType: String, errorCode: Int) = Unit
                }
            manager.discoverServices(serviceType, NsdManager.PROTOCOL_DNS_SD, listener)
            awaitClose { runCatching { manager.stopServiceDiscovery(listener) } }
        }

    @Suppress("DEPRECATION") // The replacement needs API 34; resolveService works on every supported level.
    private fun resolve(info: NsdServiceInfo, onResult: (Int?) -> Unit) {
        manager.resolveService(
            info,
            object : NsdManager.ResolveListener {
                override fun onResolveFailed(serviceInfo: NsdServiceInfo, errorCode: Int) = onResult(null)

                override fun onServiceResolved(serviceInfo: NsdServiceInfo) = onResult(serviceInfo.port)
            },
        )
    }

    companion object {
        const val CONNECT_SERVICE = "_adb-tls-connect._tcp"
        const val PAIRING_SERVICE = "_adb-tls-pairing._tcp"
    }
}
