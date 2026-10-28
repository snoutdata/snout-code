package com.snoutcode.engine.lsp;

import com.google.gson.Gson;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonNull;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

import java.io.BufferedInputStream;
import java.io.ByteArrayOutputStream;
import java.io.EOFException;
import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Duration;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicLong;

/**
 * A small LSP client over a child process's stdin and stdout: JSON-RPC framing, requests with
 * futures, and a record of what the server publishes (diagnostics, jdtls's status messages).
 * Requests from the server get the plain answers a headless client gives.
 */
public final class LspClient implements AutoCloseable {
    private static final Gson GSON = new Gson();

    private final Process process;
    private final OutputStream out;
    private final AtomicInteger ids = new AtomicInteger();
    private final Map<Integer, CompletableFuture<JsonElement>> pending = new ConcurrentHashMap<>();
    private final Thread reader;
    private final Thread memory;

    /** The latest diagnostics per document URI. */
    public final Map<String, JsonArray> diagnostics = new ConcurrentHashMap<>();
    /** When the first diagnostics arrived for each URI, in nanoseconds from {@link #startNanos}. */
    public final Map<String, Long> firstDiagnostics = new ConcurrentHashMap<>();
    /** jdtls's {@code language/status} messages, as {@code type: message}. */
    public final List<String> statuses = new CopyOnWriteArrayList<>();
    public final List<String> logMessages = new CopyOnWriteArrayList<>();
    public final CompletableFuture<Void> serviceReady = new CompletableFuture<>();
    public final long startNanos;
    private final AtomicLong lastDiagnosticsNanos = new AtomicLong();
    private final AtomicLong peakJdtlsRssKb = new AtomicLong();

    private LspClient(Process process, long startNanos) {
        this.process = process;
        this.startNanos = startNanos;
        this.out = process.getOutputStream();
        this.reader = new Thread(this::readLoop, "lsp-reader");
        this.reader.setDaemon(true);
        this.reader.start();
        this.memory = new Thread(this::sampleMemory, "lsp-memory");
        this.memory.setDaemon(true);
        this.memory.start();
    }

    /** Starts the server; its stderr goes to {@code stderrLog}. */
    public static LspClient start(List<String> command, Map<String, String> environment, File stderrLog) throws IOException {
        ProcessBuilder builder = new ProcessBuilder(command);
        builder.environment().putAll(environment);
        builder.redirectError(ProcessBuilder.Redirect.appendTo(stderrLog));
        long start = System.nanoTime();
        return new LspClient(builder.start(), start);
    }

    // ---- requests

    public JsonElement request(String method, Object params, Duration timeout) throws IOException {
        int id = ids.incrementAndGet();
        CompletableFuture<JsonElement> future = new CompletableFuture<>();
        pending.put(id, future);
        JsonObject message = new JsonObject();
        message.addProperty("jsonrpc", "2.0");
        message.addProperty("id", id);
        message.addProperty("method", method);
        message.add("params", GSON.toJsonTree(params));
        send(message);
        try {
            return future.get(timeout.toMillis(), TimeUnit.MILLISECONDS);
        } catch (TimeoutException e) {
            throw new IOException(method + " timed out after " + timeout, e);
        } catch (ExecutionException e) {
            throw new IOException(method + " failed: " + e.getCause().getMessage(), e.getCause());
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            throw new IOException(method + " interrupted", e);
        } finally {
            pending.remove(id);
        }
    }

    public void notify(String method, Object params) throws IOException {
        JsonObject message = new JsonObject();
        message.addProperty("jsonrpc", "2.0");
        message.addProperty("method", method);
        message.add("params", GSON.toJsonTree(params));
        send(message);
    }

    private synchronized void send(JsonObject message) throws IOException {
        byte[] body = message.toString().getBytes(StandardCharsets.UTF_8);
        out.write(("Content-Length: " + body.length + "\r\n\r\n").getBytes(StandardCharsets.US_ASCII));
        out.write(body);
        out.flush();
    }

    // ---- convenience

    public void didOpen(Path file) throws IOException {
        JsonObject document = new JsonObject();
        document.addProperty("uri", uri(file));
        document.addProperty("languageId", "java");
        document.addProperty("version", 1);
        document.addProperty("text", Files.readString(file));
        JsonObject params = new JsonObject();
        params.add("textDocument", document);
        notify("textDocument/didOpen", params);
    }

    public static JsonObject position(Path file, int line, int character) {
        JsonObject document = new JsonObject();
        document.addProperty("uri", uri(file));
        JsonObject position = new JsonObject();
        position.addProperty("line", line);
        position.addProperty("character", character);
        JsonObject params = new JsonObject();
        params.add("textDocument", document);
        params.add("position", position);
        return params;
    }

    public static String uri(Path file) {
        return file.toAbsolutePath().normalize().toUri().toString();
    }

    /**
     * Waits until no diagnostics have arrived for {@code quiet}, counting from now at the
     * earliest, or until {@code max} has passed.
     */
    public void awaitDiagnosticsQuiet(Duration quiet, Duration max) throws InterruptedException {
        long from = System.nanoTime();
        long deadline = from + max.toNanos();
        while (System.nanoTime() < deadline) {
            long since = System.nanoTime() - Math.max(from, lastDiagnosticsNanos.get());
            if (since >= quiet.toNanos()) {
                return;
            }
            Thread.sleep(200);
        }
    }

    /** Waits until diagnostics have arrived for each of these files, or {@code max} has passed. */
    public void awaitDiagnosticsFor(List<Path> files, Duration max) throws InterruptedException {
        long deadline = System.nanoTime() + max.toNanos();
        while (System.nanoTime() < deadline) {
            if (files.stream().allMatch(f -> firstDiagnostics.containsKey(uri(f)))) {
                return;
            }
            Thread.sleep(100);
        }
    }

    public long peakJdtlsRssKb() {
        return peakJdtlsRssKb.get();
    }

    public boolean isAlive() {
        return process.isAlive();
    }

    @Override
    public void close() throws IOException {
        try {
            if (process.isAlive()) {
                request("shutdown", JsonNull.INSTANCE, Duration.ofSeconds(30));
                notify("exit", JsonNull.INSTANCE);
            }
            if (!process.waitFor(30, TimeUnit.SECONDS)) {
                process.descendants().forEach(ProcessHandle::destroy);
                process.destroy();
            }
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        } catch (IOException e) {
            process.descendants().forEach(ProcessHandle::destroy);
            process.destroy();
        }
        memory.interrupt();
    }

    // ---- reading

    private void readLoop() {
        try (InputStream in = new BufferedInputStream(process.getInputStream())) {
            while (true) {
                int length = -1;
                String header;
                while (!(header = readLine(in)).isEmpty()) {
                    if (header.toLowerCase().startsWith("content-length:")) {
                        length = Integer.parseInt(header.substring("content-length:".length()).trim());
                    }
                }
                if (length < 0) {
                    continue;
                }
                byte[] body = in.readNBytes(length);
                if (body.length < length) {
                    throw new EOFException();
                }
                dispatch(JsonParser.parseString(new String(body, StandardCharsets.UTF_8)).getAsJsonObject());
            }
        } catch (IOException | RuntimeException e) {
            IOException closed = new IOException("the server closed the connection", e);
            pending.values().forEach(f -> f.completeExceptionally(closed));
            serviceReady.completeExceptionally(closed);
        }
    }

    private static String readLine(InputStream in) throws IOException {
        ByteArrayOutputStream line = new ByteArrayOutputStream();
        int c;
        while ((c = in.read()) != -1) {
            if (c == '\n') {
                break;
            }
            if (c != '\r') {
                line.write(c);
            }
        }
        if (c == -1 && line.size() == 0) {
            throw new EOFException();
        }
        return line.toString(StandardCharsets.US_ASCII);
    }

    private void dispatch(JsonObject message) throws IOException {
        JsonElement id = message.get("id");
        String method = message.has("method") ? message.get("method").getAsString() : null;
        if (method == null && id != null) {
            CompletableFuture<JsonElement> future = pending.get(id.getAsInt());
            if (future != null) {
                if (message.has("error")) {
                    future.completeExceptionally(new IOException(message.get("error").toString()));
                } else {
                    future.complete(message.has("result") ? message.get("result") : JsonNull.INSTANCE);
                }
            }
            return;
        }
        JsonObject params = message.has("params") && message.get("params").isJsonObject()
            ? message.getAsJsonObject("params") : new JsonObject();
        if (id != null) {
            answer(id, method, params);
            return;
        }
        switch (method) {
            case "textDocument/publishDiagnostics" -> {
                String uri = params.get("uri").getAsString();
                long now = System.nanoTime();
                diagnostics.put(uri, params.getAsJsonArray("diagnostics"));
                firstDiagnostics.putIfAbsent(uri, now - startNanos);
                lastDiagnosticsNanos.set(now);
            }
            case "language/status" -> {
                String type = params.has("type") ? params.get("type").getAsString() : "";
                statuses.add(type + ": " + (params.has("message") ? params.get("message").getAsString() : ""));
                if ("ServiceReady".equals(type)) {
                    serviceReady.complete(null);
                }
            }
            case "window/logMessage" -> logMessages.add(params.has("message") ? params.get("message").getAsString() : "");
            default -> {
            }
        }
    }

    /** Answers a request from the server the way a minimal headless client would. */
    private void answer(JsonElement id, String method, JsonObject params) throws IOException {
        JsonObject response = new JsonObject();
        response.addProperty("jsonrpc", "2.0");
        response.add("id", id);
        if ("workspace/configuration".equals(method)) {
            JsonArray result = new JsonArray();
            int items = params.has("items") ? params.getAsJsonArray("items").size() : 0;
            for (int i = 0; i < items; i++) {
                result.add(JsonNull.INSTANCE);
            }
            response.add("result", result);
        } else {
            response.add("result", JsonNull.INSTANCE);
        }
        send(response);
    }

    /** Samples the resident memory of the jdtls JVM, a descendant of the server process. */
    private void sampleMemory() {
        while (!Thread.currentThread().isInterrupted() && process.isAlive()) {
            List<Long> pids = new ArrayList<>();
            process.descendants()
                .filter(p -> p.info().commandLine().map(c -> c.contains("org.eclipse.equinox.launcher")).orElse(false))
                .forEach(p -> pids.add(p.pid()));
            for (long pid : pids) {
                try {
                    Process ps = new ProcessBuilder("ps", "-o", "rss=", "-p", Long.toString(pid)).start();
                    String text = new String(ps.getInputStream().readAllBytes(), StandardCharsets.US_ASCII).trim();
                    ps.waitFor();
                    if (!text.isEmpty()) {
                        long kb = Long.parseLong(text);
                        peakJdtlsRssKb.accumulateAndGet(kb, Math::max);
                    }
                } catch (IOException | NumberFormatException e) {
                    // the process went away between listing and sampling
                } catch (InterruptedException e) {
                    return;
                }
            }
            try {
                Thread.sleep(250);
            } catch (InterruptedException e) {
                return;
            }
        }
    }
}
