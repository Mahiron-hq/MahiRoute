// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

package io.github.mahiron_hq.mahiroute.helper;

import org.json.JSONException;
import org.json.JSONObject;

/** Identity of an audio device as the app sends it: direction, raw type and address. */
final class DeviceSpec {
    static final int ROLE_INPUT = 1;
    static final int ROLE_OUTPUT = 2;

    final boolean input;
    final int type;
    final String address;

    DeviceSpec(boolean input, int type, String address) {
        this.input = input;
        this.type = type;
        this.address = address == null ? "" : address;
    }

    int role() {
        return input ? ROLE_INPUT : ROLE_OUTPUT;
    }

    String key() {
        return (input ? "in|" : "out|") + type + "|" + address;
    }

    boolean sameAs(DeviceSpec other) {
        return other != null && key().equals(other.key());
    }

    static DeviceSpec fromJson(JSONObject json) throws JSONException {
        boolean input = "input".equals(json.getString("direction"));
        return new DeviceSpec(input, json.getInt("android_type"), json.optString("address", ""));
    }

    JSONObject toJson() throws JSONException {
        return new JSONObject()
                .put("direction", input ? "input" : "output")
                .put("android_type", type)
                .put("address", address);
    }

    @Override
    public String toString() {
        return key();
    }
}
