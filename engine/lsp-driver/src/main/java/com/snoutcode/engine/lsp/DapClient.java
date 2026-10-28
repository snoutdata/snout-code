package com.snoutcode.engine.lsp;

import com.google.gson.Gson;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

import java.io.BufferedInputStream;
import java.io.ByteArrayOutputStream;
import java.io.EOFException;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.InetSocketAddress;
import java.net.Socket;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.function.Predicate;

/**
 * A minimal Debug Adapter Protocol client over a socket, for testing java-debug headlessly:
 * requests with futures, the events received, and the reverse requests a client must answer
 * ({@code runInTerminal} is refused; the engine launches the debuggee itself).
 */
public final class DapClient implements AutoCloseable {
    private static final Gson GSON = new Gson();

    private final Socket socket;
    private final OutputStream out;
    private final AtomicInteger seq = new AtomicInteger();
    private final Map<Integer, CompletableFuture<JsonObject>> pending = new ConcurrentHashMap<>();
    /** Every event received, in order. */
    public final List<JsonObject> events = new CopyOnWriteArrayList<>();

    private DapClient(Socket socket) throws IOException {
        this.socket = socket;
        this.out = socket.getOutputStream();
        Thread reader = new Thread(this::readLoop, "dap-reader");
        reader.setDaemon(true);
        reader.start();
    }

    public static DapClient connect(String host, int port) throws IOException {
        Socket socket = new Socket();
        socket.connect(new InetSocketAddress(host, port), 10_000);
        return new DapClient(socket);
    }

    /** Sends a request and waits for its response; fails when the response is not a success. */
    public JsonObject request(String command, Object arguments, Duration timeout) throws IOException {
        JsonObject response = send(command, arguments, timeout);
        if (!response.get("success").getAsBoolean()) {
            throw new IOException(command + " failed: " + response);
        }
        return response.has("body") && response.get("body").isJsonObject() ? response.getAsJsonObject("body") : new JsonObject();
    }

    /** Sends a request and returns the whole response, success or not. */
    public JsonObject send(String command, Object arguments, Duration timeout) throws IOException {
        int id = seq.incrementAndGet();
        CompletableFuture<JsonObject> future = new CompletableFuture<>();
        pending.put(id, future);
        JsonObject message = new JsonObject();
        message.addProperty("seq", id);
        message.addProperty("type", "request");
        message.addProperty("command", command);
        if (arguments != null) {
            message.add("arguments", GSON.toJsonTree(arguments));
        }
        write(message);
        try {
            return future.get(timeout.toMillis(), TimeUnit.MILLISECONDS);
        } catch (TimeoutException e) {
            throw new IOException(command + " timed out after " + timeout, e);
        } catch (ExecutionException e) {
            throw new IOException(command + " failed", e.getCause());
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            throw new IOException(command + " interrupted", e);
        } finally {
            pending.remove(id);
        }
    }

    /** Waits for an event of this name that matches, received at index {@code from} or later. */
    public JsonObject awaitEvent(String event, int from, Predicate<JsonObject> match, Duration timeout)
        throws IOException, InterruptedException {
        long deadline = System.nanoTime() + timeout.toNanos();
        while (System.nanoTime() < deadline) {
            for (int i = from; i < events.size(); i++) {
                JsonObject e = events.get(i);
                JsonObject body = e.has("body") && e.get("body").isJsonObject() ? e.getAsJsonObject("body") : new JsonObject();
                if (event.equals(e.get("event").getAsString()) && match.test(body)) {
                    return body;
                }
            }
            Thread.sleep(20);
        }
        throw new IOException("no " + event + " event within " + timeout + "; events: " + events);
    }

    private synchronized void write(JsonObject message) throws IOException {
        byte[] body = message.toString().getBytes(StandardCharsets.UTF_8);
        out.write(("Content-Length: " + body.length + "\r\n\r\n").getBytes(StandardCharsets.US_ASCII));
        out.write(body);
        out.flush();
    }

    private void readLoop() {
        try (InputStream in = new BufferedInputStream(socket.getInputStream())) {
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
            IOException closed = new IOException("the debug adapter closed the connection", e);
            pending.values().forEach(f -> f.completeExceptionally(closed));
        }
    }

    private void dispatch(JsonObject message) throws IOException {
        String type = message.get("type").getAsString();
        switch (type) {
            case "response" -> {
                CompletableFuture<JsonObject> future = pending.get(message.get("request_seq").getAsInt());
                if (future != null) {
                    future.complete(message);
                }
            }
            case "event" -> events.add(message);
            case "request" -> {
                JsonObject response = new JsonObject();
                response.addProperty("seq", seq.incrementAndGet());
                response.addProperty("type", "response");
                response.add("request_seq", message.get("seq"));
                response.add("command", message.get("command"));
                response.addProperty("success", false);
                response.addProperty("message", "not supported by this client");
                write(response);
            }
            default -> {
            }
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
        if (c == -1) {
            throw new EOFException();
        }
        return line.toString(StandardCharsets.US_ASCII);
    }

    @Override
    public void close() throws IOException {
        socket.close();
    }
}
