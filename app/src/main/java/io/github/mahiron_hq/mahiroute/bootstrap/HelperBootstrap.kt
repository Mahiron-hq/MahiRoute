// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

package io.github.mahiron_hq.mahiroute.bootstrap

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import android.os.SystemClock
import android.provider.Settings
import android.util.Log
import io.github.mahiron_hq.mahiroute.core.EngineHost
import io.github.mahiron_hq.mahiroute.core.Native
import io.github.mahiron_hq.mahiroute.notifications.Notifications
import java.io.File
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.filterNotNull
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull
import org.json.JSONObject

/** What the helper start is doing right now. */
enum class Step {
    Preparing,

    /** Only the user can switch wireless debugging on; the start continues by itself afterwards. */
    WaitingForDebugging,
    FindingService,

    /** The service has not shown up yet, most likely because Android's prompt is still open. */
    WaitingForConfirmation,
    StartingHelper,
    FindingPairingDialog,
    Pairing,
}

/** Why the helper could not be started without the user's help. */
enum class Problem {
    NoWifi,
    WirelessDebuggingOff,
    ServiceNotFound,
    Unreachable,
    Other,
}

/** Progress of starting the shell-privileged helper. */
sealed interface BootstrapState {
    data object Idle : BootstrapState

    data class Working(val step: Step) : BootstrapState

    /** The app is not paired yet; [lastCodeRejected] tells whether a code was just refused. */
    data class NeedsPairing(val lastCodeRejected: Boolean, val dialogMissing: Boolean) : BootstrapState

    data class Failed(val problem: Problem, val detail: String?) : BootstrapState
}

/**
 * Starts the helper on the phone itself through wireless debugging: finds the service, pairs once
 * with a code the user types, and runs the helper through the Rust ADB client.
 */
object HelperBootstrap {
    private const val TAG = "MahiRoute"
    private const val PAIRING_WAIT_MS = 10 * 60_000L
    private const val DEBUGGING_WAIT_MS = 5 * 60_000L
    private const val DEBUGGING_POLL_MS = 500L
    private const val PAIRING_SERVICE_WAIT_MS = 8_000L
    private const val PEER_NAME = "MahiRoute"
    private const val KEY_FILE = "adb_key.pem"

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
    private val mutableState = MutableStateFlow<BootstrapState>(BootstrapState.Idle)
    private var job: Job? = null
    private var pendingCode: CompletableDeferred<String>? = null

    val state: StateFlow<BootstrapState> = mutableState.asStateFlow()

    /** Starts the procedure unless it is already running. */
    @Synchronized
    fun start(context: Context) {
        if (job?.isActive == true) return
        val app = context.applicationContext
        job = scope.launch { run(app) }
    }

    /** Hands the six-digit pairing code the user typed to the waiting procedure. */
    fun submitPairingCode(code: String) {
        val digits = code.filter { it.isDigit() }
        if (digits.length == PAIRING_CODE_LENGTH) pendingCode?.complete(digits)
    }

    /** Abandons a running procedure. */
    @Synchronized
    fun cancel(context: Context) {
        job?.cancel()
        job = null
        pendingCode = null
        mutableState.value = BootstrapState.Idle
        Notifications.cancelPairingPrompt(context)
    }

    private suspend fun run(context: Context) {
        try {
            mutableState.value = BootstrapState.Working(Step.Preparing)
            if (!isOnWifi(context)) return fail(Problem.NoWifi, null)
            if (!enableWirelessDebugging(context) && !waitForDebuggingSwitchedOn(context)) {
                return fail(Problem.WirelessDebuggingOff, null)
            }
            val discovery = AdbDiscovery(context)

            var started = startHelper(context, discovery)
            if (started is Started.NotPaired) {
                if (!pairWithUser(context, discovery)) return
                started = startHelper(context, discovery)
            }
            when (started) {
                is Started.Ready -> {
                    EngineHost.connect(started.port, started.token)
                    HelperEndpointStore.save(context, started.port, started.token)
                    mutableState.value = BootstrapState.Idle
                }
                is Started.NotPaired -> fail(Problem.Other, "pairing did not take effect")
                is Started.Failed -> fail(started.problem, started.detail)
            }
        } finally {
            Notifications.cancelPairingPrompt(context)
        }
    }

    private suspend fun startHelper(context: Context, discovery: AdbDiscovery): Started {
        val keeper =
            DebuggingKeeper(
                clock = SystemClock::elapsedRealtime,
                appInForeground = { AppForeground.isResumed },
                isOn = { isDebuggingOn(context) },
                turnOn = { enableWirelessDebugging(context) },
            )
        return StartLoop(
            clock = SystemClock::elapsedRealtime,
            pause = { delay(it) },
            keepEnabled = keeper::tick,
            findPort = { timeout, refused -> discovery.findPort(AdbDiscovery.CONNECT_SERVICE, timeout, refused) },
            start = { port -> requestHelperStart(context, port) },
            onStep = { mutableState.value = BootstrapState.Working(it) },
        ).run()
    }

    private fun requestHelperStart(context: Context, port: Int): Started {
        val answer =
            JSONObject(
                Native.adbStartHelper(port, context.applicationInfo.sourceDir, keyPath(context), context.packageName),
            )
        return when {
            answer.has("port") -> Started.Ready(answer.getInt("port"), answer.getString("token"))
            answer.optString("kind") == "not_paired" -> Started.NotPaired
            answer.optString("kind") == "not_enabled" -> Started.Failed(Problem.WirelessDebuggingOff, answer.optString("error"))
            answer.optString("kind") == "unreachable" -> Started.Failed(Problem.Unreachable, answer.optString("error"))
            else -> Started.Failed(Problem.Other, answer.optString("error"))
        }
    }

    /**
     * Without the secure-settings permission (before the first successful start) only the user can
     * switch wireless debugging on, so wait for that instead of failing and asking for a second tap.
     */
    private suspend fun waitForDebuggingSwitchedOn(context: Context): Boolean {
        mutableState.value = BootstrapState.Working(Step.WaitingForDebugging)
        return withTimeoutOrNull(DEBUGGING_WAIT_MS) {
            while (!isDebuggingOn(context)) delay(DEBUGGING_POLL_MS)
            true
        } ?: false
    }

    /** Waits for a pairing code and pairs; false when the procedure ended without success. */
    private suspend fun pairWithUser(context: Context, discovery: AdbDiscovery): Boolean {
        // The pairing service only exists while the dialog in Settings is open, so keep watching
        // for it from the moment the code is requested and pair the instant it is entered.
        val advertised = MutableStateFlow<Int?>(null)
        val watcher = scope.launch { discovery.watch(AdbDiscovery.PAIRING_SERVICE).collect { advertised.value = it } }
        try {
            return pairLoop(context, advertised)
        } finally {
            watcher.cancel()
        }
    }

    private suspend fun pairLoop(context: Context, advertised: StateFlow<Int?>): Boolean {
        var rejected = false
        var dialogMissing = false
        while (true) {
            val deferred = CompletableDeferred<String>()
            pendingCode = deferred
            mutableState.value = BootstrapState.NeedsPairing(rejected, dialogMissing)
            Notifications.showPairingPrompt(context)
            val code = withTimeoutOrNull(PAIRING_WAIT_MS) { deferred.await() }
            if (code == null) {
                fail(Problem.Other, "no pairing code was entered")
                return false
            }
            mutableState.value = BootstrapState.Working(Step.FindingPairingDialog)
            val port = withTimeoutOrNull(PAIRING_SERVICE_WAIT_MS) { advertised.filterNotNull().first() }
            if (port == null) {
                dialogMissing = true
                continue
            }
            dialogMissing = false
            mutableState.value = BootstrapState.Working(Step.Pairing)
            val answer = JSONObject(Native.adbPair(port, code, keyPath(context), PEER_NAME))
            if (answer.optBoolean("ok")) return true
            Log.w(TAG, "pairing failed: ${answer.optString("error")}")
            if (answer.optString("kind") == "wrong_code") {
                rejected = true
            } else {
                fail(Problem.Other, answer.optString("error"))
                return false
            }
        }
    }

    private fun fail(problem: Problem, detail: String?) {
        Log.w(TAG, "helper start failed: $problem $detail")
        mutableState.value = BootstrapState.Failed(problem, detail)
    }

    private fun keyPath(context: Context): String = File(context.filesDir, KEY_FILE).absolutePath

    private fun isOnWifi(context: Context): Boolean {
        val manager = context.getSystemService(ConnectivityManager::class.java)
        val capabilities = manager.getNetworkCapabilities(manager.activeNetwork) ?: return false
        return capabilities.hasTransport(NetworkCapabilities.TRANSPORT_WIFI)
    }

    private fun isDebuggingOn(context: Context): Boolean =
        Settings.Global.getInt(context.contentResolver, ADB_WIFI_ENABLED, 0) == 1

    /** Switches wireless debugging on when the app may; otherwise reports whether it already is on. */
    private fun enableWirelessDebugging(context: Context): Boolean {
        val resolver = context.contentResolver
        if (isDebuggingOn(context)) return true
        val allowed =
            context.checkSelfPermission(Manifest.permission.WRITE_SECURE_SETTINGS) == PackageManager.PERMISSION_GRANTED
        if (!allowed) return false
        Settings.Global.putInt(resolver, DEVELOPMENT_SETTINGS_ENABLED, 1)
        return Settings.Global.putInt(resolver, ADB_WIFI_ENABLED, 1)
    }

    private const val PAIRING_CODE_LENGTH = 6
    private const val ADB_WIFI_ENABLED = "adb_wifi_enabled"
    private const val DEVELOPMENT_SETTINGS_ENABLED = "development_settings_enabled"
}
