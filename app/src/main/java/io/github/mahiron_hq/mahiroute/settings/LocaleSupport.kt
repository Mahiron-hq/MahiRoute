// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

package io.github.mahiron_hq.mahiroute.settings

import android.content.Context
import android.content.res.Configuration
import android.os.LocaleList
import java.util.Locale

/** Applies the language chosen in the app to a context; the system language is left alone. */
object LocaleSupport {
    fun wrap(base: Context): Context {
        val tag = AppSettings.languageChoice(base).tag ?: return base
        val locale = Locale.forLanguageTag(tag)
        val configuration = Configuration(base.resources.configuration)
        configuration.setLocales(LocaleList(locale))
        return base.createConfigurationContext(configuration)
    }
}
