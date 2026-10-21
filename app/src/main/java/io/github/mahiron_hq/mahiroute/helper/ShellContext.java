// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

package io.github.mahiron_hq.mahiroute.helper;

import android.content.AttributionSource;
import android.content.Context;
import android.content.ContextWrapper;
import android.os.Process;

/**
 * A context that presents the helper as the shell package, so audio streams it opens carry an
 * attribution (uid and package) the audio server accepts. The system context of a bare
 * {@code app_process} names the package "android", which Android 14 and later reject for uid 2000.
 */
final class ShellContext extends ContextWrapper {
    private static final String SHELL_PACKAGE = "com.android.shell";

    private final AttributionSource attribution =
            new AttributionSource.Builder(Process.myUid()).setPackageName(SHELL_PACKAGE).build();

    ShellContext(Context base) {
        super(base);
    }

    @Override
    public String getPackageName() {
        return SHELL_PACKAGE;
    }

    /** Hidden in the public SDK, but the audio classes read it reflectively through the framework. */
    public String getOpPackageName() {
        return SHELL_PACKAGE;
    }

    @Override
    public AttributionSource getAttributionSource() {
        return attribution;
    }

    @Override
    public Context getApplicationContext() {
        return this;
    }
}
