// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

package io.github.mahiron_hq.mahiroute.helper;

import android.annotation.SuppressLint;
import android.content.Context;
import android.media.AudioAttributes;
import android.media.AudioFormat;
import android.media.AudioRecord;
import android.media.AudioDeviceInfo;
import android.media.AudioManager;
import android.media.AudioPlaybackConfiguration;
import android.media.AudioRecordingConfiguration;
import android.os.Build;
import java.lang.reflect.Constructor;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * Privileged audio routing: preferred devices per capture preset and product strategy, and
 * hiding devices from the audio policy. Everything here needs the shell identity.
 */
@SuppressLint({"PrivateApi", "DiscouragedPrivateApi", "SoonBlockedPrivateApi"})
final class AudioRouter {
    /** MIC, CAMCORDER, VOICE_RECOGNITION, VOICE_COMMUNICATION, UNPROCESSED, VOICE_PERFORMANCE. */
    private static final int[] CAPTURE_PRESETS = {1, 5, 6, 7, 9, 10};
    private static final int CONNECTION_UNAVAILABLE = 0;
    private static final int CONNECTION_AVAILABLE = 1;
    private static final long MASKED_PORT_BASE = 0x80000000L;
    private static final int ALL_DEVICES = AudioManager.GET_DEVICES_INPUTS | AudioManager.GET_DEVICES_OUTPUTS;

    private final AudioManager audio;
    private final Context shellContext;
    private final StateStore state;
    private final Constructor<?> attributesConstructor;
    private final Method attributesType;
    private final Method attributesAddress;
    private final Method setCapturePreferred;
    private final Method getCapturePreferred;
    private final Method clearCapturePreferred;
    private final Method setStrategyPreferred;
    private final Method getStrategyPreferred;
    private final Method removeStrategyPreferred;
    private final Method setConnectionState;
    private final List<Object> strategies = new ArrayList<>();


    AudioRouter(Context context, AudioManager audio, StateStore state) throws ReflectiveOperationException {
        this.audio = audio;
        this.shellContext = new ShellContext(context);
        this.state = state;
        Class<?> attributes = Class.forName("android.media.AudioDeviceAttributes");
        attributesConstructor = attributes.getConstructor(int.class, int.class, String.class);
        attributesType = attributes.getMethod("getType");
        attributesAddress = attributes.getMethod("getAddress");
        setCapturePreferred = AudioManager.class.getMethod(
                "setPreferredDeviceForCapturePreset", int.class, attributes);
        getCapturePreferred = AudioManager.class.getMethod("getPreferredDevicesForCapturePreset", int.class);
        clearCapturePreferred = AudioManager.class.getMethod("clearPreferredDevicesForCapturePreset", int.class);
        Class<?> strategy = Class.forName("android.media.audiopolicy.AudioProductStrategy");
        setStrategyPreferred = AudioManager.class.getMethod("setPreferredDeviceForStrategy", strategy, attributes);
        getStrategyPreferred = AudioManager.class.getMethod("getPreferredDeviceForStrategy", strategy);
        removeStrategyPreferred = AudioManager.class.getMethod("removePreferredDeviceForStrategy", strategy);
        Class<?> system = Class.forName("android.media.AudioSystem");
        setConnectionState = system.getMethod("setDeviceConnectionState", attributes, int.class, int.class);
        for (Object item : (List<?>) strategy.getMethod("getAudioProductStrategies").invoke(null)) {
            String text = String.valueOf(item);
            // Internal strategies carry no user-visible audio and must be left alone.
            if (!text.contains("REROUTING") && !text.contains("STRATEGY_PATCH")) {
                strategies.add(item);
            }
        }
    }

    /** Brings back devices a previous helper left hidden. */
    synchronized void restoreStaleMasks(List<String> warnings) {
        for (StateStore.Mask entry : state.masks()) {
            unmask(entry, warnings);
        }
    }

    synchronized int mode() {
        return audio.getMode();
    }

    synchronized JSONArray listDevices() throws JSONException {
        JSONArray result = new JSONArray();
        Set<String> visible = new HashSet<>();
        for (AudioDeviceInfo device : audio.getDevices(ALL_DEVICES)) {
            DeviceSpec spec = specOf(device);
            visible.add(spec.key());
            result.put(deviceJson(device.getId(), spec, String.valueOf(device.getProductName()), false));
        }
        long index = 0;
        for (StateStore.Mask entry : state.masks()) {
            if (!visible.contains(entry.spec.key())) {
                result.put(deviceJson(MASKED_PORT_BASE + index, entry.spec, entry.name, true));
            }
            index++;
        }
        return result;
    }

    /** Converges to the desired state; returns the problems met on the way. */
    synchronized JSONArray apply(JSONObject desired) throws JSONException {
        List<String> warnings = new ArrayList<>();
        DeviceSpec input = desired.isNull("prefer_input") ? null : DeviceSpec.fromJson(desired.getJSONObject("prefer_input"));
        DeviceSpec output = desired.isNull("prefer_output") ? null : DeviceSpec.fromJson(desired.getJSONObject("prefer_output"));
        Map<String, DeviceSpec> masked = new HashMap<>();
        JSONArray maskList = desired.optJSONArray("masked");
        for (int i = 0; maskList != null && i < maskList.length(); i++) {
            DeviceSpec spec = DeviceSpec.fromJson(maskList.getJSONObject(i));
            masked.put(spec.key(), spec);
        }
        for (StateStore.Mask entry : state.masks()) {
            if (!masked.containsKey(entry.spec.key())) {
                unmask(entry, warnings);
            }
        }
        applyInput(input, warnings);
        applyOutput(output, warnings);
        for (DeviceSpec spec : masked.values()) {
            // A recorded mask is stale when the device is visible again, e.g. after a reboot.
            if (!state.isMasked(spec) || find(spec) != null) {
                mask(spec, warnings);
            }
        }
        return new JSONArray(warnings);
    }

    /** Removes every preference and unmasks every device. */
    synchronized JSONArray clearAll() {
        List<String> warnings = new ArrayList<>();
        for (StateStore.Mask entry : state.masks()) {
            unmask(entry, warnings);
        }
        applyInput(null, warnings);
        applyOutput(null, warnings);
        return new JSONArray(warnings);
    }

    /** Puts the output preference back after the framework cleared it, e.g. on a mode change. */
    synchronized void reassertOutput() {
        if (state.output() != null) {
            applyOutput(state.output(), new ArrayList<>());
        }
    }

    synchronized JSONArray streams() throws JSONException {
        JSONArray result = new JSONArray();
        for (AudioRecordingConfiguration config : audio.getActiveRecordingConfigurations()) {
            AudioDeviceInfo device = config.getAudioDevice();
            result.put(streamJson("record", clientUid(config), config.getClientAudioSource(), device));
        }
        for (AudioPlaybackConfiguration config : audio.getActivePlaybackConfigurations()) {
            AudioAttributes attributes = config.getAudioAttributes();
            result.put(streamJson("playback", clientUid(config), attributes.getUsage(), playbackDevice(config)));
        }
        return result;
    }

    /**
     * Opens a short silent stream and reports where the system routed it.
     *
     * @param input true to record with audio source {@code kind}, false to play with usage {@code kind}
     */
    // The helper runs as the shell user, which holds RECORD_AUDIO, so the app-level permission check does not apply.
    @SuppressLint("MissingPermission")
    synchronized JSONObject probe(long id, boolean input, int kind) throws JSONException {
        JSONObject response = new JSONObject().put("id", id);
        AudioDeviceInfo routed;
        Integer level = null;
        try {
            if (input) {
                int rate = 16000;
                int size = Math.max(AudioRecord.getMinBufferSize(rate, AudioFormat.CHANNEL_IN_MONO,
                        AudioFormat.ENCODING_PCM_16BIT), 4096);
                AudioRecord record = new AudioRecord.Builder()
                        .setContext(shellContext)
                        .setAudioSource(kind)
                        .setAudioFormat(new AudioFormat.Builder().setSampleRate(rate)
                                .setEncoding(AudioFormat.ENCODING_PCM_16BIT)
                                .setChannelMask(AudioFormat.CHANNEL_IN_MONO).build())
                        .setBufferSizeInBytes(size * 2)
                        .build();
                try {
                    if (record.getState() != AudioRecord.STATE_INITIALIZED) {
                        return response.put("result", "error").put("message", "probe recorder not initialized");
                    }
                    record.startRecording();
                    short[] samples = new short[rate / 10];
                    double sum = 0;
                    long count = 0;
                    for (int i = 0; i < 3; i++) {
                        int read = record.read(samples, 0, samples.length);
                        for (int j = 0; j < read; j++) {
                            sum += (double) samples[j] * samples[j];
                        }
                        count += Math.max(read, 0);
                    }
                    routed = record.getRoutedDevice();
                    level = count == 0 ? 0 : (int) Math.sqrt(sum / count);
                    record.stop();
                } finally {
                    record.release();
                }
            } else {
                if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) {
                    return response.put("result", "error").put("message", "checking playback routing needs Android 13");
                }
                // The policy states which device a stream with these attributes would get, so no
                // stream has to be opened (and no attribution is needed) to check playback routing.
                List<AudioDeviceInfo> devices = audio.getAudioDevicesForAttributes(
                        new AudioAttributes.Builder().setUsage(kind).build());
                routed = devices.isEmpty() ? null : devices.get(0);
            }
        } catch (RuntimeException e) {
            return response.put("result", "error").put("message", "probe failed: " + e);
        }
        response.put("result", "probed");
        response.put("routed_port", routed == null ? JSONObject.NULL : routed.getId());
        response.put("routed_type", routed == null ? JSONObject.NULL : routed.getType());
        response.put("routed_address", routed == null ? JSONObject.NULL : routed.getAddress());
        response.put("level", level == null ? JSONObject.NULL : level);
        return response;
    }

    private void applyInput(DeviceSpec want, List<String> warnings) {
        try {
            if (want == null) {
                if (state.input() != null) {
                    for (int preset : CAPTURE_PRESETS) {
                        if (capturePreferenceIs(preset, state.input())) {
                            clearCapturePreferred.invoke(audio, preset);
                        }
                    }
                }
                state.setInput(null);
                return;
            }
            if (find(want) == null) {
                warnings.add("input device not visible: " + want);
                return;
            }
            Object attributes = attributes(want);
            for (int preset : CAPTURE_PRESETS) {
                if (!capturePreferenceIs(preset, want)
                        && !Boolean.TRUE.equals(setCapturePreferred.invoke(audio, preset, attributes))) {
                    warnings.add("capture preset " + preset + " rejected " + want);
                }
            }
            state.setInput(want);
        } catch (ReflectiveOperationException e) {
            warnings.add("input routing failed: " + describe(e));
        }
    }

    private void applyOutput(DeviceSpec want, List<String> warnings) {
        try {
            if (want == null) {
                if (state.output() != null) {
                    for (Object strategy : strategies) {
                        if (matches(getStrategyPreferred.invoke(audio, strategy), state.output())) {
                            removeStrategyPreferred.invoke(audio, strategy);
                        }
                    }
                }
                state.setOutput(null);
                return;
            }
            if (find(want) == null) {
                warnings.add("output device not visible: " + want);
                return;
            }
            Object attributes = attributes(want);
            for (Object strategy : strategies) {
                if (!matches(getStrategyPreferred.invoke(audio, strategy), want)
                        && !Boolean.TRUE.equals(setStrategyPreferred.invoke(audio, strategy, attributes))) {
                    warnings.add("strategy rejected " + want);
                }
            }
            state.setOutput(want);
        } catch (ReflectiveOperationException e) {
            warnings.add("output routing failed: " + describe(e));
        }
    }

    private void mask(DeviceSpec spec, List<String> warnings) {
        AudioDeviceInfo device = find(spec);
        if (device == null) {
            warnings.add("cannot hide a device that is not visible: " + spec);
            return;
        }
        // Persist first: a crash between the call and the write must not leak a hidden device.
        state.addMask(spec, String.valueOf(device.getProductName()));
        if (!setConnection(spec, CONNECTION_UNAVAILABLE, warnings)) {
            state.removeMask(spec);
        }
    }

    private void unmask(StateStore.Mask entry, List<String> warnings) {
        // The policy forgets hidden devices on a reboot and then refuses to restore a visible one.
        if (find(entry.spec) != null || setConnection(entry.spec, CONNECTION_AVAILABLE, warnings)) {
            state.removeMask(entry.spec);
        }
    }

    private boolean setConnection(DeviceSpec spec, int state, List<String> warnings) {
        try {
            Object result = setConnectionState.invoke(null, attributes(spec), state, 0);
            if (result instanceof Integer && (Integer) result == 0) {
                return true;
            }
            warnings.add("connection state " + state + " for " + spec + " returned " + result);
        } catch (ReflectiveOperationException e) {
            warnings.add("connection state failed: " + describe(e));
        }
        return false;
    }

    private boolean capturePreferenceIs(int preset, DeviceSpec spec) throws ReflectiveOperationException {
        List<?> current = (List<?>) getCapturePreferred.invoke(audio, preset);
        return current != null && current.size() == 1 && matches(current.get(0), spec);
    }

    private boolean matches(Object attributes, DeviceSpec spec) throws ReflectiveOperationException {
        if (attributes == null || spec == null) {
            return false;
        }
        int type = (Integer) attributesType.invoke(attributes);
        String address = String.valueOf(attributesAddress.invoke(attributes));
        return type == spec.type && address.equals(spec.address);
    }

    private Object attributes(DeviceSpec spec) throws ReflectiveOperationException {
        return attributesConstructor.newInstance(spec.role(), spec.type, spec.address);
    }

    private AudioDeviceInfo find(DeviceSpec spec) {
        for (AudioDeviceInfo device : audio.getDevices(ALL_DEVICES)) {
            if (specOf(device).sameAs(spec)) {
                return device;
            }
        }
        return null;
    }

    private static DeviceSpec specOf(AudioDeviceInfo device) {
        return new DeviceSpec(device.isSource(), device.getType(), device.getAddress());
    }

    private static JSONObject deviceJson(long port, DeviceSpec spec, String name, boolean masked) throws JSONException {
        return new JSONObject()
                .put("port", port)
                .put("input", spec.input)
                .put("type", spec.type)
                .put("address", spec.address)
                .put("name", name)
                .put("masked", masked);
    }

    private static JSONObject streamJson(String kind, int uid, int source, AudioDeviceInfo device) throws JSONException {
        JSONObject json = new JSONObject().put("kind", kind).put("uid", uid).put("source", source);
        json.put("device_port", device == null ? JSONObject.NULL : device.getId());
        json.put("device_type", device == null ? JSONObject.NULL : device.getType());
        return json;
    }

    private static int clientUid(Object configuration) {
        try {
            return (Integer) configuration.getClass().getMethod("getClientUid").invoke(configuration);
        } catch (ReflectiveOperationException | ClassCastException e) {
            return -1;
        }
    }

    private static AudioDeviceInfo playbackDevice(AudioPlaybackConfiguration configuration) {
        try {
            return (AudioDeviceInfo) configuration.getClass().getMethod("getAudioDeviceInfo").invoke(configuration);
        } catch (ReflectiveOperationException | ClassCastException e) {
            return null;
        }
    }

    private static String describe(ReflectiveOperationException e) {
        Throwable cause = e instanceof InvocationTargetException && e.getCause() != null ? e.getCause() : e;
        return cause.toString();
    }
}
