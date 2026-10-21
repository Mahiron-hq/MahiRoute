// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

package io.github.mahiron_hq.mahiroute.bootstrap

import android.app.Activity
import android.app.Application
import android.os.Bundle

/**
 * Tells whether one of the app's screens is in the foreground. A system prompt that covers the app
 * pauses the screen, so this turns false while Android asks something and true again once it is answered.
 */
object AppForeground : Application.ActivityLifecycleCallbacks {
    @Volatile
    var isResumed: Boolean = false
        private set

    fun register(app: Application) = app.registerActivityLifecycleCallbacks(this)

    override fun onActivityResumed(activity: Activity) {
        isResumed = true
    }

    override fun onActivityPaused(activity: Activity) {
        isResumed = false
    }

    override fun onActivityCreated(activity: Activity, savedInstanceState: Bundle?) = Unit

    override fun onActivityStarted(activity: Activity) = Unit

    override fun onActivityStopped(activity: Activity) = Unit

    override fun onActivitySaveInstanceState(activity: Activity, outState: Bundle) = Unit

    override fun onActivityDestroyed(activity: Activity) = Unit
}
