// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

package io.github.mahiron_hq.mahiroute.helper;

import android.os.Build;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.Socket;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import org.json.JSONException;
import org.json.JSONObject;

/** One authenticated connection from the app: reads request lines, writes response lines. */
final class Session implements Runnable {
    private static final int MAX_LINE_BYTES = 1 << 20;
    private static final int AUTH_TIMEOUT_MS = 5_000;

    private final Socket socket;
    private final byte[] token;
    private final Object writeLock = new Object();
    private volatile boolean authenticated;

    Session(Socket socket, String token) {
        this.socket = socket;
        this.token = token.getBytes(StandardCharsets.UTF_8);
    }

    void close() {
        try {
            socket.close();
        } catch (IOException ignored) {
            // Closing a dead socket needs no handling.
        }
    }

    void sendEvent(JSONObject event) {
        if (authenticated) {
            write(event);
        }
    }

    @Override
    public void run() {
        try (InputStream in = socket.getInputStream()) {
            // A connection that does not introduce itself soon is not the app; do not let it linger.
            socket.setSoTimeout(AUTH_TIMEOUT_MS);
            String line;
            while ((line = readLine(in)) != null) {
                JSONObject request;
                try {
                    request = new JSONObject(line);
                } catch (JSONException e) {
                    return;
                }
                if (!handle(request)) {
                    return;
                }
            }
        } catch (IOException e) {
            // The app went away; the routing state is kept for the next connection.
        } finally {
            close();
        }
    }

    /** Returns false when the connection must be dropped. */
    private boolean handle(JSONObject request) {
        long id = request.optLong("id", -1);
        String op = request.optString("op", "");
        try {
            if (!authenticated) {
                return authenticate(id, op, request);
            }
            switch (op) {
                case "list_devices":
                    write(response(id, "devices").put("devices", HelperMain.router().listDevices()));
                    break;
                case "apply":
                    write(response(id, "applied")
                            .put("warnings", HelperMain.router().apply(request.getJSONObject("desired"))));
                    break;
                case "clear_all":
                    write(response(id, "cleared").put("warnings", HelperMain.router().clearAll()));
                    break;
                case "diagnostics":
                    write(response(id, "diagnostics").put("streams", HelperMain.router().streams()));
                    break;
                case "probe":
                    write(HelperMain.router()
                            .probe(id, "input".equals(request.optString("direction")), request.optInt("kind", 0)));
                    break;
                case "shutdown":
                    write(response(id, "cleared"));
                    HelperMain.shutdown();
                    break;
                default:
                    write(error(id, "unknown operation " + op));
            }
        } catch (JSONException | RuntimeException e) {
            write(error(id, String.valueOf(e)));
        }
        return true;
    }

    private boolean authenticate(long id, String op, JSONObject request) throws JSONException {
        String given = request.optString("token", "");
        boolean ok = "hello".equals(op)
                && MessageDigest.isEqual(token, given.getBytes(StandardCharsets.UTF_8))
                && request.optInt("version", -1) == HelperMain.PROTOCOL_VERSION;
        if (!ok) {
            write(error(id, "authentication failed"));
            return false;
        }
        try {
            socket.setSoTimeout(0);
        } catch (IOException e) {
            return false;
        }
        authenticated = true;
        HelperMain.promote(this);
        write(response(id, "hello")
                .put("version", HelperMain.PROTOCOL_VERSION)
                .put("sdk", Build.VERSION.SDK_INT)
                .put("fingerprint", Build.FINGERPRINT)
                .put("uid", android.os.Process.myUid()));
        return true;
    }

    private static JSONObject response(long id, String result) throws JSONException {
        return new JSONObject().put("id", id).put("result", result);
    }

    private static JSONObject error(long id, String message) {
        try {
            return response(id, "error").put("message", message);
        } catch (JSONException e) {
            throw new IllegalStateException(e);
        }
    }

    private void write(JSONObject message) {
        byte[] bytes = (message.toString() + "\n").getBytes(StandardCharsets.UTF_8);
        synchronized (writeLock) {
            try {
                OutputStream out = socket.getOutputStream();
                out.write(bytes);
                out.flush();
            } catch (IOException e) {
                close();
            }
        }
    }

    /** Reads one line without its terminator; null at end of stream. */
    private static String readLine(InputStream in) throws IOException {
        ByteArrayOutputStream buffer = new ByteArrayOutputStream();
        int next;
        while ((next = in.read()) != -1) {
            if (next == '\n') {
                return new String(buffer.toByteArray(), StandardCharsets.UTF_8);
            }
            if (buffer.size() >= MAX_LINE_BYTES) {
                throw new IOException("request line too long");
            }
            buffer.write(next);
        }
        return buffer.size() == 0 ? null : new String(buffer.toByteArray(), StandardCharsets.UTF_8);
    }
}
