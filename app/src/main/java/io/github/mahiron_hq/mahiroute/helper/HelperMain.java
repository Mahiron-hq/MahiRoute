// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

package io.github.mahiron_hq.mahiroute.helper;

import android.annotation.SuppressLint;
import android.content.Context;
import android.media.AudioDeviceCallback;
import android.media.AudioDeviceInfo;
import android.media.AudioManager;
import android.media.AudioPlaybackConfiguration;
import android.media.AudioRecordingConfiguration;
import android.os.Handler;
import android.os.Looper;
import java.io.File;
import java.io.IOException;
import java.net.InetAddress;
import java.net.ServerSocket;
import java.net.Socket;
import java.util.ArrayList;
import java.util.List;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * Entry point of the shell-privileged helper. It is started with {@code app_process} from the
 * app's own APK, listens on a loopback port and executes routing requests from the app.
 *
 * <p>Usage: {@code HelperMain --token <secret> [--port <n>] [--state <file>]}.
 */
// The helper is a plain process started through app_process: its static fields are not app components.
@SuppressLint({"PrivateApi", "DiscouragedPrivateApi", "StaticFieldLeak"})
public final class HelperMain {
    static final int PROTOCOL_VERSION = 1;
    private static final int SHELL_UID = 2000;
    private static final int ROOT_UID = 0;
    private static final String DEFAULT_STATE_FILE = "/data/local/tmp/mahiroute-state.json";
    private static final long[] REASSERT_DELAYS_MS = {0, 60, 250, 1000};

    private static AudioRouter router;
    private static Handler mainHandler;
    private static volatile Session session;

    private HelperMain() {
    }

    public static void main(String[] args) throws Exception {
        String token = null;
        int port = 0;
        String statePath = DEFAULT_STATE_FILE;
        for (int i = 0; i + 1 < args.length; i += 2) {
            switch (args[i]) {
                case "--token":
                    token = args[i + 1];
                    break;
                case "--port":
                    port = Integer.parseInt(args[i + 1]);
                    break;
                case "--state":
                    statePath = args[i + 1];
                    break;
                default:
                    System.err.println("unknown argument " + args[i]);
                    System.exit(2);
            }
        }
        if (token == null || token.length() < 16) {
            System.err.println("a --token of at least 16 characters is required");
            System.exit(2);
        }
        int uid = android.os.Process.myUid();
        if (uid != SHELL_UID && uid != ROOT_UID) {
            System.err.println("helper must run as shell, running as uid " + uid);
            System.exit(3);
        }

        Looper.prepareMainLooper();
        Class<?> activityThread = Class.forName("android.app.ActivityThread");
        Object thread = activityThread.getMethod("systemMain").invoke(null);
        Context context = (Context) activityThread.getMethod("getSystemContext").invoke(thread);
        AudioManager audio = (AudioManager) context.getSystemService(Context.AUDIO_SERVICE);
        mainHandler = new Handler(Looper.getMainLooper());

        router = new AudioRouter(context, audio, new StateStore(new File(statePath)));
        List<String> startupWarnings = new ArrayList<>();
        router.restoreStaleMasks(startupWarnings);
        router.clearAll();
        for (String warning : startupWarnings) {
            System.err.println("startup: " + warning);
        }

        registerCallbacks(audio);
        Runtime.getRuntime().addShutdownHook(new Thread(() -> router.clearAll()));

        ServerSocket server = new ServerSocket(port, 2, InetAddress.getByAddress(new byte[] {127, 0, 0, 1}));
        System.out.println("MAHIROUTE_READY " + server.getLocalPort());
        System.out.flush();
        final String sessionToken = token;
        Thread acceptor = new Thread(() -> acceptLoop(server, sessionToken), "mahiroute-accept");
        acceptor.setDaemon(true);
        acceptor.start();
        Looper.loop();
    }

    static AudioRouter router() {
        return router;
    }

    static void shutdown() {
        router.clearAll();
        System.exit(0);
    }

    /**
     * Makes an authenticated connection the current one and drops the one it replaces. Only a
     * connection that proved the token may do this, so a stray connect cannot evict the app.
     */
    static synchronized void promote(Session next) {
        Session previous = session;
        session = next;
        if (previous != null && previous != next) {
            previous.close();
        }
    }

    /** Pushes an event to the connected app, if any. */
    static void broadcast(JSONObject event) {
        Session current = session;
        if (current != null) {
            current.sendEvent(event);
        }
    }

    private static void acceptLoop(ServerSocket server, String token) {
        while (true) {
            try {
                Socket socket = server.accept();
                Session next = new Session(socket, token);
                Thread thread = new Thread(next, "mahiroute-session");
                thread.setDaemon(true);
                thread.start();
            } catch (IOException e) {
                System.err.println("accept failed: " + e);
                return;
            }
        }
    }

    private static void registerCallbacks(AudioManager audio) {
        Debouncer devices = new Debouncer(150, () -> {
            try {
                broadcast(new JSONObject().put("event", "devices").put("devices", router.listDevices()));
            } catch (JSONException e) {
                System.err.println("devices event failed: " + e);
            }
        });
        Debouncer streams = new Debouncer(250, () -> {
            try {
                broadcast(new JSONObject().put("event", "streams").put("streams", router.streams()));
            } catch (JSONException e) {
                System.err.println("streams event failed: " + e);
            }
        });
        audio.registerAudioDeviceCallback(new AudioDeviceCallback() {
            @Override
            public void onAudioDevicesAdded(AudioDeviceInfo[] added) {
                devices.poke();
            }

            @Override
            public void onAudioDevicesRemoved(AudioDeviceInfo[] removed) {
                devices.poke();
            }
        }, mainHandler);
        audio.registerAudioRecordingCallback(new AudioManager.AudioRecordingCallback() {
            @Override
            public void onRecordingConfigChanged(List<AudioRecordingConfiguration> configs) {
                streams.poke();
            }
        }, mainHandler);
        audio.registerAudioPlaybackCallback(new AudioManager.AudioPlaybackCallback() {
            @Override
            public void onPlaybackConfigChanged(List<AudioPlaybackConfiguration> configs) {
                streams.poke();
            }
        }, mainHandler);
        audio.addOnModeChangedListener(mainHandler::post, mode -> {
            try {
                broadcast(new JSONObject().put("event", "mode").put("mode", mode));
            } catch (JSONException e) {
                System.err.println("mode event failed: " + e);
            }
            // The framework clears the communication strategy's preference when the mode changes.
            for (long delay : REASSERT_DELAYS_MS) {
                mainHandler.postDelayed(() -> router.reassertOutput(), delay);
            }
        });
    }

    /** Coalesces bursts of callbacks into one action on the main thread. */
    private static final class Debouncer {
        private final long delayMs;
        private final Runnable action;

        Debouncer(long delayMs, Runnable action) {
            this.delayMs = delayMs;
            this.action = action;
        }

        void poke() {
            mainHandler.removeCallbacks(action);
            mainHandler.postDelayed(action, delayMs);
        }
    }

    /** Builds the JSON array of warnings returned by a failed bulk operation. */
    static JSONArray warnings(List<String> items) {
        return new JSONArray(items);
    }
}
