package com.snoutcode.engine.cli;

import com.google.gson.JsonElement;
import com.google.gson.JsonNull;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

import java.io.BufferedInputStream;
import java.io.BufferedOutputStream;
import java.io.EOFException;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.io.PrintStream;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.atomic.AtomicInteger;

/**
 * Sits between the app and jdtls on the LSP stream. Every message passes through unchanged,
 * except:
 *
 * <ul>
 *   <li>requests and notifications from the app whose method starts with {@code snout/}, which
 *       the {@link Engine} answers;</li>
 *   <li>responses to the requests the engine itself sends jdtls (string ids {@code snout-<n>},
 *       which never collide with the app's), which go back to the engine.</li>
 * </ul>
 *
 * The engine's notifications go to the app between jdtls's messages, never inside one.
 */
final class LspProxy implements Engine.Jdtls {
    private final InputStream appIn;
    private final BufferedOutputStream appOut;
    private final Process jdtls;
    private final OutputStream jdtlsIn;
    private final PrintStream log;
    private final Map<String, CompletableFuture<JsonElement>> pending = new ConcurrentHashMap<>();
    private final AtomicInteger ids = new AtomicInteger();
    private final CompletableFuture<Void> initialized = new CompletableFuture<>();
    private final CompletableFuture<Void> serviceReady = new CompletableFuture<>();
    private Engine engine;
    private volatile SpringSupport spring;

    LspProxy(InputStream appIn, OutputStream appOut, Process jdtls, PrintStream log) {
        this.appIn = new BufferedInputStream(appIn);
        this.appOut = appOut instanceof BufferedOutputStream b ? b : new BufferedOutputStream(appOut);
        this.jdtls = jdtls;
        this.jdtlsIn = new BufferedOutputStream(jdtls.getOutputStream());
        this.log = log;
    }

    void attach(Engine engine) {
        this.engine = engine;
    }

    /** Spring support: it sees the app's messages and jdtls's diagnostics and responses first. */
    void attach(SpringSupport spring) {
        this.spring = spring;
    }

    /** Runs until the app closes the stream or jdtls exits; returns jdtls's exit code. */
    int run() throws InterruptedException {
        Thread fromJdtls = new Thread(this::pumpJdtls, "jdtls-to-app");
        fromJdtls.setDaemon(true);
        fromJdtls.start();
        Thread fromApp = new Thread(this::pumpApp, "app-to-jdtls");
        fromApp.setDaemon(true);
        fromApp.start();
        int exit = jdtls.waitFor();
        fromJdtls.join(2000);
        IOException closed = new IOException("jdtls exited");
        pending.values().forEach(f -> f.completeExceptionally(closed));
        return exit;
    }

    // ---- app to jdtls

    private void pumpApp() {
        try {
            while (true) {
                JsonObject message = Rpc.read(appIn);
                String method = message.has("method") ? message.get("method").getAsString() : null;
                if (method != null && method.startsWith("snout/")) {
                    engine.receive(message);
                    continue;
                }
                if ("textDocument/didOpen".equals(method) || "textDocument/didClose".equals(method)) {
                    engine.documentEvent(method, message);
                }
                SpringSupport s = spring;
                if (s != null && s.fromApp(message)) {
                    continue;
                }
                toJdtls(message);
                if ("initialized".equals(method)) {
                    initialized.complete(null);
                }
            }
        } catch (EOFException e) {
            // the app is gone
        } catch (IOException | RuntimeException e) {
            log.println("snout-engine: reading from the app failed: " + SnoutEngine.describe(e));
        }
        try {
            jdtlsIn.close();
        } catch (IOException e) {
            // jdtls is gone too
        }
        // jdtls ends once its input closes; should it not, it is stopped.
        try {
            if (!jdtls.waitFor(10, java.util.concurrent.TimeUnit.SECONDS)) {
                log.println("snout-engine: jdtls did not end with its input; stopping it");
                Lsp.stopProcess(jdtls, 5);
            }
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        }
    }

    private void toJdtls(JsonObject message) throws IOException {
        synchronized (jdtlsIn) {
            Rpc.write(jdtlsIn, message);
        }
    }

    // ---- jdtls to app

    private void pumpJdtls() {
        try (InputStream in = new BufferedInputStream(jdtls.getInputStream())) {
            while (true) {
                JsonObject message = Rpc.read(in);
                JsonElement id = message.get("id");
                if (!message.has("method") && id != null && id.isJsonPrimitive() && id.getAsJsonPrimitive().isString()
                    && id.getAsString().startsWith("snout-")) {
                    CompletableFuture<JsonElement> future = pending.remove(id.getAsString());
                    if (future != null) {
                        if (message.has("error")) {
                            future.completeExceptionally(new IOException(message.get("error").toString()));
                        } else {
                            future.complete(message.has("result") ? message.get("result") : JsonNull.INSTANCE);
                        }
                    }
                    continue;
                }
                if ("language/status".equals(method(message)) && message.get("params").isJsonObject()) {
                    JsonElement type = message.getAsJsonObject("params").get("type");
                    if (type != null && "ServiceReady".equals(type.getAsString()) && !serviceReady.isDone()) {
                        serviceReady.complete(null);
                        engine.jdtlsReady();
                        SpringSupport s = spring;
                        if (s != null) {
                            s.start();
                        }
                    }
                }
                SpringSupport s = spring;
                if (s != null) {
                    if (!message.has("method") && s.jdtlsResponse(message)) {
                        continue;
                    }
                    if ("textDocument/publishDiagnostics".equals(method(message)) && message.get("params").isJsonObject()) {
                        message = s.jdtlsDiagnostics(message);
                    }
                }
                if ("textDocument/publishDiagnostics".equals(method(message)) && message.get("params").isJsonObject()) {
                    // Held back while the file's Scala is not in jdtls yet.
                    message = engine.diagnostics(message);
                    if (message == null) {
                        continue;
                    }
                }
                toApp(message);
            }
        } catch (EOFException e) {
            // jdtls exited
        } catch (IOException | RuntimeException e) {
            log.println("snout-engine: reading from jdtls failed: " + SnoutEngine.describe(e));
        }
    }

    private static String method(JsonObject message) {
        return message.has("method") ? message.get("method").getAsString() : null;
    }

    /** Sends a message to the app; safe from any thread. */
    void toApp(JsonObject message) {
        write(appOut, message);
    }

    /**
     * Writes a message to the app's stream, between whole messages: the engine writes to it before
     * jdtls (and this proxy) starts, too.
     */
    static void write(BufferedOutputStream out, JsonObject message) {
        synchronized (out) {
            try {
                Rpc.write(out, message);
            } catch (IOException e) {
                // The app is gone; jdtls will see its input close.
            }
        }
    }

    // ---- the engine's own requests to jdtls

    @Override
    public CompletableFuture<JsonElement> request(String method, JsonElement params) {
        String id = "snout-" + ids.incrementAndGet();
        CompletableFuture<JsonElement> future = new CompletableFuture<>();
        pending.put(id, future);
        initialized.thenRun(() -> {
            try {
                toJdtls(Rpc.request(new JsonPrimitive(id), method, params));
            } catch (IOException e) {
                pending.remove(id);
                future.completeExceptionally(e);
            }
        });
        return future;
    }

    @Override
    public CompletableFuture<Void> ready() {
        return serviceReady;
    }
}
