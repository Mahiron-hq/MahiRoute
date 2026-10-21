// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

package io.github.mahiron_hq.mahiroute.helper;

import android.annotation.SuppressLint;
import android.bluetooth.BluetoothAdapter;
import android.bluetooth.BluetoothClass;
import android.bluetooth.BluetoothDevice;
import android.bluetooth.BluetoothManager;
import android.content.Context;
import android.os.ParcelUuid;
import android.util.Log;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.List;

/**
 * Bluetooth headphones that are connected but that Android has deactivated: when a wired headset
 * is plugged in, the Bluetooth stack drops the active A2DP device, so the audio framework stops
 * listing it although the link is still up. Only the shell identity may bring such a device back.
 */
@SuppressLint({"PrivateApi", "DiscouragedPrivateApi", "SoonBlockedPrivateApi", "MissingPermission"})
final class BluetoothSink {
    /** {@code BluetoothAdapter.ACTIVE_DEVICE_ALL}: the device carries both media and calls. */
    private static final int ACTIVE_DEVICE_ALL = 2;

    /** A connected audio device as the Bluetooth stack sees it. */
    static final class Headphones {
        final String address;
        final String name;

        Headphones(String address, String name) {
            this.address = address;
            this.name = name;
        }
    }

    private static final ParcelUuid A2DP_SINK = ParcelUuid.fromString("0000110b-0000-1000-8000-00805f9b34fb");
    private static final String TAG = "MahiRoute";

    private final BluetoothAdapter adapter;
    private final Method isConnected;
    private final Method setActiveDevice;

    private BluetoothSink(BluetoothAdapter adapter, Method isConnected, Method setActiveDevice) {
        this.adapter = adapter;
        this.isConnected = isConnected;
        this.setActiveDevice = setActiveDevice;
    }

    /** Returns null when this phone has no usable Bluetooth stack or the hidden calls are missing. */
    static BluetoothSink open(Context context) {
        try {
            registerMainlineServices();
            // The manager takes its attribution from the context, which has to name the shell package.
            BluetoothManager manager = BluetoothManager.class.getConstructor(Context.class).newInstance(context);
            BluetoothAdapter adapter = manager.getAdapter();
            if (adapter == null) {
                return null;
            }
            Method isConnected = BluetoothDevice.class.getMethod("isConnected");
            Method setActive = BluetoothAdapter.class.getMethod("setActiveDevice", BluetoothDevice.class, int.class);
            return new BluetoothSink(adapter, isConnected, setActive);
        } catch (ReflectiveOperationException | RuntimeException e) {
            Log.w(TAG, "Bluetooth is not usable from the helper", e);
            return null;
        }
    }

    /** A bare app_process never registers the Bluetooth service manager that the adapter looks up. */
    private static void registerMainlineServices() throws ReflectiveOperationException {
        try {
            Class.forName("android.app.ActivityThread").getMethod("initializeMainlineModules").invoke(null);
        } catch (InvocationTargetException e) {
            // A second registration is refused, which means the services are already in place.
            if (!(e.getCause() instanceof IllegalStateException)) {
                throw e;
            }
        }
    }

    /** Connected Bluetooth audio devices, active or not. */
    List<Headphones> connected() {
        List<Headphones> result = new ArrayList<>();
        try {
            for (BluetoothDevice device : adapter.getBondedDevices()) {
                if (playsAudio(device) && Boolean.TRUE.equals(isConnected.invoke(device))) {
                    result.add(new Headphones(device.getAddress(), device.getName()));
                }
            }
        } catch (ReflectiveOperationException | RuntimeException e) {
            Log.w(TAG, "listing Bluetooth devices failed", e);
            return new ArrayList<>();
        }
        return result;
    }

    /** Many headphones report no device class, so the A2DP sink service record is checked as well. */
    private static boolean playsAudio(BluetoothDevice device) {
        BluetoothClass type = device.getBluetoothClass();
        if (type != null && type.getMajorDeviceClass() == BluetoothClass.Device.Major.AUDIO_VIDEO) {
            return true;
        }
        ParcelUuid[] services = device.getUuids();
        if (services != null) {
            for (ParcelUuid service : services) {
                if (A2DP_SINK.equals(service)) {
                    return true;
                }
            }
        }
        return false;
    }

    /** Makes the device the active one for media and calls; returns false when the stack refuses. */
    boolean activate(String address) {
        try {
            BluetoothDevice device = adapter.getRemoteDevice(address);
            return Boolean.TRUE.equals(setActiveDevice.invoke(adapter, device, ACTIVE_DEVICE_ALL));
        } catch (ReflectiveOperationException | RuntimeException e) {
            Log.w(TAG, "activating a Bluetooth device failed", e);
            return false;
        }
    }
}
