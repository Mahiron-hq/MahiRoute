// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

package io.github.mahiron_hq.mahiroute.helper;

import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * Remembers on disk what the helper changed in the audio policy: hidden devices and the
 * preferences it set. A restarted helper uses it to undo the work of a predecessor that was
 * killed without cleaning up.
 */
final class StateStore {
    /** A hidden device together with the product name it had when it was hidden. */
    static final class Mask {
        final DeviceSpec spec;
        final String name;

        Mask(DeviceSpec spec, String name) {
            this.spec = spec;
            this.name = name == null ? "" : name;
        }
    }

    private final File file;
    private final Map<String, Mask> masks = new LinkedHashMap<>();
    private DeviceSpec input;
    private DeviceSpec output;

    StateStore(File file) {
        this.file = file;
        load();
    }

    List<Mask> masks() {
        return new ArrayList<>(masks.values());
    }

    boolean isMasked(DeviceSpec spec) {
        return masks.containsKey(spec.key());
    }

    void addMask(DeviceSpec spec, String name) {
        masks.put(spec.key(), new Mask(spec, name));
        save();
    }

    void removeMask(DeviceSpec spec) {
        if (masks.remove(spec.key()) != null) {
            save();
        }
    }

    DeviceSpec input() {
        return input;
    }

    DeviceSpec output() {
        return output;
    }

    void setInput(DeviceSpec spec) {
        if (sameSpec(input, spec)) {
            return;
        }
        input = spec;
        save();
    }

    void setOutput(DeviceSpec spec) {
        if (sameSpec(output, spec)) {
            return;
        }
        output = spec;
        save();
    }

    private static boolean sameSpec(DeviceSpec a, DeviceSpec b) {
        return a == null ? b == null : a.sameAs(b);
    }

    private void load() {
        if (!file.exists()) {
            return;
        }
        try (FileInputStream in = new FileInputStream(file)) {
            ByteArrayOutputStream buffer = new ByteArrayOutputStream();
            byte[] chunk = new byte[4096];
            int read;
            while ((read = in.read(chunk)) > 0) {
                buffer.write(chunk, 0, read);
            }
            JSONObject root = new JSONObject(new String(buffer.toByteArray(), StandardCharsets.UTF_8));
            JSONArray array = root.optJSONArray("masked");
            for (int i = 0; array != null && i < array.length(); i++) {
                JSONObject item = array.getJSONObject(i);
                DeviceSpec spec = DeviceSpec.fromJson(item);
                masks.put(spec.key(), new Mask(spec, item.optString("name", "")));
            }
            input = root.isNull("input") || !root.has("input") ? null : DeviceSpec.fromJson(root.getJSONObject("input"));
            output = root.isNull("output") || !root.has("output") ? null : DeviceSpec.fromJson(root.getJSONObject("output"));
        } catch (IOException | JSONException e) {
            // A damaged file only loses the ability to undo a predecessor's changes.
            masks.clear();
            input = null;
            output = null;
        }
    }

    private void save() {
        try {
            JSONArray array = new JSONArray();
            for (Mask mask : masks.values()) {
                array.put(mask.spec.toJson().put("name", mask.name));
            }
            JSONObject root = new JSONObject().put("masked", array);
            root.put("input", input == null ? JSONObject.NULL : input.toJson());
            root.put("output", output == null ? JSONObject.NULL : output.toJson());
            try (FileOutputStream out = new FileOutputStream(file)) {
                out.write(root.toString().getBytes(StandardCharsets.UTF_8));
            }
        } catch (IOException | JSONException e) {
            System.err.println("helper state not saved: " + e);
        }
    }
}
