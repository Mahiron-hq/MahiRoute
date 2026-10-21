// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

package io.github.mahiron_hq.mahiroute.settings

import android.content.Context
import androidx.core.content.edit
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

enum class ThemeMode(val stored: String) {
    System("system"),
    Light("light"),
    Dark("dark");

    companion object {
        fun fromStored(value: String?): ThemeMode = entries.firstOrNull { it.stored == value } ?: System
    }
}

/** The interface language: the system one, or a fixed one chosen in the app. */
enum class LanguageChoice(val stored: String, val tag: String?) {
    System("system", null),
    Russian("ru", "ru"),
    English("en", "en");

    companion object {
        fun fromStored(value: String?): LanguageChoice = entries.firstOrNull { it.stored == value } ?: System
    }
}

/**
 * Whether the stored acceptance covers the privacy notice that is current now. A newer notice has
 * to be accepted again; a missing acceptance is version 0.
 */
fun needsConsent(acceptedVersion: Int, currentVersion: Int): Boolean = acceptedVersion < currentVersion

/**
 * Preferences that belong to the interface. They live in plain shared preferences because the
 * language has to be known before any screen or notification text is created.
 */
object AppSettings {
    /** Bump when the text of the privacy notice changes in substance. */
    const val POLICY_VERSION = 1

    private const val PREFS = "settings"
    private const val KEY_THEME = "theme"
    private const val KEY_LANGUAGE = "language"
    private const val KEY_ALERTS = "alerts"
    private const val KEY_POLICY = "policy_version"

    private val mutableTheme = MutableStateFlow(ThemeMode.System)
    private val mutableLanguage = MutableStateFlow(LanguageChoice.System)
    private val mutableAlerts = MutableStateFlow(true)
    private val mutableAcceptedPolicy = MutableStateFlow(0)

    val theme: StateFlow<ThemeMode> = mutableTheme.asStateFlow()
    val language: StateFlow<LanguageChoice> = mutableLanguage.asStateFlow()
    val alerts: StateFlow<Boolean> = mutableAlerts.asStateFlow()
    val acceptedPolicy: StateFlow<Int> = mutableAcceptedPolicy.asStateFlow()

    private fun prefs(context: Context) = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    /** Reads the stored values into the observable state; call once per process. */
    fun load(context: Context) {
        mutableTheme.value = ThemeMode.fromStored(prefs(context).getString(KEY_THEME, null))
        mutableLanguage.value = languageChoice(context)
        mutableAlerts.value = alertsEnabled(context)
        mutableAcceptedPolicy.value = prefs(context).getInt(KEY_POLICY, 0)
    }

    fun languageChoice(context: Context): LanguageChoice =
        LanguageChoice.fromStored(prefs(context).getString(KEY_LANGUAGE, null))

    fun alertsEnabled(context: Context): Boolean = prefs(context).getBoolean(KEY_ALERTS, true)

    fun setTheme(context: Context, mode: ThemeMode) {
        prefs(context).edit { putString(KEY_THEME, mode.stored) }
        mutableTheme.value = mode
    }

    fun setLanguage(context: Context, choice: LanguageChoice) {
        prefs(context).edit { putString(KEY_LANGUAGE, choice.stored) }
        mutableLanguage.value = choice
    }

    fun setAlerts(context: Context, enabled: Boolean) {
        prefs(context).edit { putBoolean(KEY_ALERTS, enabled) }
        mutableAlerts.value = enabled
    }

    fun acceptPolicy(context: Context) {
        prefs(context).edit { putInt(KEY_POLICY, POLICY_VERSION) }
        mutableAcceptedPolicy.value = POLICY_VERSION
    }

    /** Back to the defaults; the accepted privacy notice stays accepted. */
    fun reset(context: Context) {
        setTheme(context, ThemeMode.System)
        setLanguage(context, LanguageChoice.System)
        setAlerts(context, true)
    }
}
