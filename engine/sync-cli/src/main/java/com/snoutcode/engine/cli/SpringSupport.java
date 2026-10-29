package com.snoutcode.engine.cli;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonNull;
import com.google.gson.JsonObject;

import java.io.PrintStream;
import java.net.URI;
import java.nio.file.Path;
import java.util.LinkedHashSet;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.ScheduledFuture;
import java.util.concurrent.TimeUnit;
import java.util.function.Consumer;

/**
 * The proxy's half of the Spring support; the jdtls bundle {@code com.snoutcode.engine.spring}
 * holds the other, with the model. This half:
 *
 * <ul>
 *   <li>scans once jdtls is ready, and asks the bundle to read Java files again as the app
 *       edits them (debounced), and the Scala bridge's classes after a Scala compile;</li>
 *   <li>adds the Spring diagnostics of a file to jdtls's own when jdtls publishes them, and
 *       publishes again when only the Spring ones change;</li>
 *   <li>adds Spring quick fixes to jdtls's code actions, and {@code @Value} values to its inlay hints;</li>
 *   <li>owns the {@code application*.properties} and {@code application*.yml} documents, which
 *       jdtls ignores: their text, completion, hover, definition and validation.</li>
 * </ul>
 */
final class SpringSupport {
    /** How the proxy hands a jdtls response back: now (the same or a new message), or later through {@code toApp}. */
    interface Responder {
        void respond(JsonObject message);
    }

    private final LspProxy proxy;
    private final Consumer<JsonObject> toApp;
    private final PrintStream log;
    private final ScheduledExecutorService timer = Executors.newSingleThreadScheduledExecutor(r -> {
        Thread t = new Thread(r, "snout-spring");
        t.setDaemon(true);
        return t;
    });
    private final java.util.concurrent.ExecutorService pool = Executors.newCachedThreadPool(r -> {
        Thread t = new Thread(r, "snout-spring-request");
        t.setDaemon(true);
        return t;
    });
    /** Properties document path to its text. */
    private final Map<String, String> documents = new ConcurrentHashMap<>();
    /** Path to jdtls's last diagnostics, and to the Spring ones. */
    private final Map<String, JsonArray> jdtlsDiagnostics = new ConcurrentHashMap<>();
    private final Map<String, JsonArray> springDiagnostics = new ConcurrentHashMap<>();
    private final Map<String, String> uris = new ConcurrentHashMap<>();
    /** App request id to the kind of jdtls response to add to: codeAction, inlayHint. */
    private final Map<String, JsonObject> pendingRequests = new ConcurrentHashMap<>();
    private final Set<String> dirty = new LinkedHashSet<>();
    private ScheduledFuture<?> flush;
    private final Map<String, ScheduledFuture<?>> validations = new ConcurrentHashMap<>();
    private final CompletableFuture<Void> scanned = new CompletableFuture<>();
    private volatile boolean available = true;

    SpringSupport(LspProxy proxy, Consumer<JsonObject> toApp, PrintStream log) {
        this.proxy = proxy;
        this.toApp = toApp;
        this.log = log;
    }

    static boolean isPropertiesFile(String uri) {
        String name = uri.substring(uri.lastIndexOf('/') + 1).toLowerCase(Locale.ROOT);
        return name.startsWith("application") && (name.endsWith(".properties") || name.endsWith(".yml") || name.endsWith(".yaml"))
            || name.startsWith("bootstrap") && (name.endsWith(".properties") || name.endsWith(".yml") || name.endsWith(".yaml"));
    }

    static String key(String uri) {
        try {
            if (uri.startsWith("file:")) {
                return Path.of(URI.create(uri)).toAbsolutePath().normalize().toString();
            }
        } catch (RuntimeException e) {
            // not a file URI
        }
        return uri;
    }

    // ---- startup

    /** Called once jdtls has imported the projects. */
    void start() {
        pool.submit(() -> {
            try {
                long start = System.nanoTime();
                JsonElement result = command("snout.spring.scan", new JsonObject(), 1800);
                JsonObject r = result.getAsJsonObject();
                apply(r.getAsJsonObject("diagnostics"));
                log.println("snout-engine: Spring model ready in " + (System.nanoTime() - start) / 1_000_000 + " ms: " + r.get("stats"));
            } catch (Exception e) {
                available = false;
                log.println("snout-engine: no Spring support: " + SnoutEngine.describe(e));
            } finally {
                scanned.complete(null);
                for (String path : documents.keySet()) {
                    validate(uris.getOrDefault(path, path), 0);
                }
            }
        });
    }

    void shutdown() {
        timer.shutdownNow();
        pool.shutdownNow();
    }

    private JsonElement command(String command, JsonObject params, int timeoutSeconds) throws Exception {
        JsonObject p = new JsonObject();
        p.addProperty("command", command);
        JsonArray args = new JsonArray();
        args.add(params.toString());
        p.add("arguments", args);
        return proxy.request("workspace/executeCommand", p).get(timeoutSeconds, TimeUnit.SECONDS);
    }

    /**
     * {@code snout/springSetProfiles}: how profiles are evaluated. Publishes the diagnostics that
     * change with them, and checks the open application files again.
     */
    JsonElement setProfiles(JsonObject params) throws Exception {
        JsonElement result = request("snout.spring.setProfiles", params);
        apply(result.getAsJsonObject().getAsJsonObject("diagnostics"));
        revalidateDocuments();
        JsonObject out = result.getAsJsonObject().deepCopy();
        out.remove("diagnostics");
        return out.has("profiles") ? out.get("profiles") : out;
    }

    /** An application file was saved: the profiles are evaluated again, as they are set. */
    private void refreshProfiles() {
        try {
            if (!scanned.isDone() || !available) {
                return;
            }
            JsonElement result = command("snout.spring.refreshProfiles", new JsonObject(), 600);
            apply(result.getAsJsonObject().getAsJsonObject("diagnostics"));
            revalidateDocuments();
        } catch (Exception e) {
            log.println("snout-engine: Spring profiles failed: " + SnoutEngine.describe(e));
        }
    }

    private void revalidateDocuments() {
        for (String path : documents.keySet()) {
            validate(uris.getOrDefault(path, path), 0);
        }
    }

    /** A request for the engine's own methods ({@code snout/springBeans} and the others). */
    JsonElement request(String command, JsonObject params) throws Exception {
        proxy.ready().get(30, TimeUnit.MINUTES);
        scanned.get(30, TimeUnit.MINUTES);
        if (!available) {
            throw new Engine.RequestError(-32603, "Spring support is not loaded");
        }
        return command(command, params, 600);
    }

    // ---- from the app

    /** Takes a message from the app; true when it is the support's alone and must not reach jdtls. */
    boolean fromApp(JsonObject message) {
        String method = message.has("method") ? message.get("method").getAsString() : null;
        if (method == null) {
            return false;
        }
        JsonObject params = message.has("params") && message.get("params").isJsonObject() ? message.getAsJsonObject("params") : new JsonObject();
        String uri = uri(params);
        if (uri != null && isPropertiesFile(uri)) {
            return propertiesMessage(method, message, params, uri);
        }
        if (uri != null && uri.endsWith(".java")) {
            switch (method) {
                case "textDocument/didChange", "textDocument/didSave", "textDocument/didOpen" -> touch(uri);
                case "textDocument/codeAction", "textDocument/inlayHint" -> {
                    if (message.has("id") && available) {
                        JsonObject pending = new JsonObject();
                        pending.addProperty("method", method);
                        pending.add("params", params);
                        pendingRequests.put(message.get("id").toString(), pending);
                    }
                }
                default -> {
                    // passes through
                }
            }
        }
        if ("workspace/didChangeWatchedFiles".equals(method) && params.has("changes")) {
            for (JsonElement c : params.getAsJsonArray("changes")) {
                String changed = c.getAsJsonObject().get("uri").getAsString();
                if (changed.endsWith(".java") || changed.endsWith(".xml")) {
                    touch(changed);
                }
            }
        }
        return false;
    }

    private static String uri(JsonObject params) {
        JsonElement doc = params.get("textDocument");
        if (doc != null && doc.isJsonObject() && doc.getAsJsonObject().has("uri")) {
            return doc.getAsJsonObject().get("uri").getAsString();
        }
        return null;
    }

    private void touch(String uri) {
        if (!available) {
            return;
        }
        synchronized (dirty) {
            dirty.add(uri);
            if (flush != null) {
                flush.cancel(false);
            }
            flush = timer.schedule(this::flush, 400, TimeUnit.MILLISECONDS);
        }
    }

    /** After the Scala bridge compiled: its classes changed. */
    void scalaCompiled() {
        touch("file:///snout-scala-bridge.scala");
    }

    private void flush() {
        Set<String> batch;
        synchronized (dirty) {
            batch = new LinkedHashSet<>(dirty);
            dirty.clear();
        }
        if (batch.isEmpty()) {
            return;
        }
        pool.submit(() -> {
            try {
                scanned.get(30, TimeUnit.MINUTES);
                if (!available) {
                    return;
                }
                JsonObject params = new JsonObject();
                JsonArray list = new JsonArray();
                batch.forEach(list::add);
                params.add("uris", list);
                JsonElement result = command("snout.spring.update", params, 600);
                apply(result.getAsJsonObject().getAsJsonObject("diagnostics"));
            } catch (Exception e) {
                log.println("snout-engine: Spring update failed: " + SnoutEngine.describe(e));
            }
        });
    }

    // ---- diagnostics

    private void apply(JsonObject diagnostics) {
        if (diagnostics == null) {
            return;
        }
        for (Map.Entry<String, JsonElement> e : diagnostics.entrySet()) {
            String path = key(e.getKey());
            uris.putIfAbsent(path, e.getKey());
            JsonArray d = e.getValue().getAsJsonArray();
            if (d.isEmpty()) {
                springDiagnostics.remove(path);
            } else {
                springDiagnostics.put(path, d);
            }
            publish(path);
        }
    }

    private void publish(String path) {
        JsonArray all = new JsonArray();
        JsonArray own = jdtlsDiagnostics.get(path);
        if (own != null) {
            all.addAll(own);
        }
        JsonArray spring = springDiagnostics.get(path);
        if (spring != null) {
            all.addAll(spring);
        }
        JsonObject params = new JsonObject();
        params.addProperty("uri", uris.getOrDefault(path, path));
        params.add("diagnostics", all);
        toApp.accept(Rpc.notification("textDocument/publishDiagnostics", params));
    }

    /** jdtls published a file's diagnostics: keep them, and add the Spring ones. */
    JsonObject jdtlsDiagnostics(JsonObject message) {
        JsonObject params = message.getAsJsonObject("params");
        String uri = params.get("uri").getAsString();
        String path = key(uri);
        uris.put(path, uri);
        JsonArray own = params.has("diagnostics") ? params.getAsJsonArray("diagnostics") : new JsonArray();
        jdtlsDiagnostics.put(path, own);
        JsonArray spring = springDiagnostics.get(path);
        if (spring == null || spring.isEmpty()) {
            return message;
        }
        JsonObject copy = message.deepCopy();
        JsonArray all = new JsonArray();
        all.addAll(own);
        all.addAll(spring);
        copy.getAsJsonObject("params").add("diagnostics", all);
        return copy;
    }

    // ---- jdtls's responses to the app

    /**
     * jdtls answered an app request: adds the Spring code actions or inlay hints. True when the
     * support took the response and sends it itself.
     */
    boolean jdtlsResponse(JsonObject message) {
        if (pendingRequests.isEmpty() || !message.has("id")) {
            return false;
        }
        JsonObject pending = pendingRequests.remove(message.get("id").toString());
        if (pending == null) {
            return false;
        }
        String method = pending.get("method").getAsString();
        JsonObject params = pending.getAsJsonObject("params");
        if ("textDocument/codeAction".equals(method)) {
            JsonArray fixes = codeActions(params);
            if (fixes.isEmpty() || message.has("error")) {
                return false;
            }
            JsonObject copy = message.deepCopy();
            JsonArray result = copy.has("result") && copy.get("result").isJsonArray() ? copy.getAsJsonArray("result") : new JsonArray();
            JsonArray merged = new JsonArray();
            merged.addAll(fixes);
            merged.addAll(result);
            copy.add("result", merged);
            copy.remove("error");
            toApp.accept(copy);
            return true;
        }
        if ("textDocument/inlayHint".equals(method)) {
            pool.submit(() -> {
                JsonObject copy = message.deepCopy();
                try {
                    JsonObject p = new JsonObject();
                    p.addProperty("uri", uri(params));
                    p.add("range", params.get("range"));
                    JsonElement hints = command("snout.spring.inlayHints", p, 30);
                    if (hints != null && hints.isJsonArray() && !hints.getAsJsonArray().isEmpty()) {
                        JsonArray merged = new JsonArray();
                        if (copy.has("result") && copy.get("result").isJsonArray()) {
                            merged.addAll(copy.getAsJsonArray("result"));
                        }
                        merged.addAll(hints.getAsJsonArray());
                        copy.add("result", merged);
                        copy.remove("error");
                    }
                } catch (Exception e) {
                    // jdtls's hints alone
                }
                toApp.accept(copy);
            });
            return true;
        }
        return false;
    }

    /** The quick fixes of the Spring diagnostics that touch the requested range. */
    private JsonArray codeActions(JsonObject params) {
        JsonArray out = new JsonArray();
        String uri = uri(params);
        JsonArray spring = uri == null ? null : springDiagnostics.get(key(uri));
        if (spring == null) {
            return out;
        }
        JsonObject range = params.getAsJsonObject("range");
        int from = range.getAsJsonObject("start").get("line").getAsInt();
        int to = range.getAsJsonObject("end").get("line").getAsInt();
        for (JsonElement e : spring) {
            JsonObject d = e.getAsJsonObject();
            int line = d.getAsJsonObject("range").getAsJsonObject("start").get("line").getAsInt();
            if (line < from || line > to || !d.has("data")) {
                continue;
            }
            for (JsonElement f : d.getAsJsonObject("data").getAsJsonArray("fixes")) {
                JsonObject fix = f.getAsJsonObject();
                JsonObject action = new JsonObject();
                action.addProperty("title", fix.get("title").getAsString());
                action.addProperty("kind", "quickfix");
                JsonArray diagnostics = new JsonArray();
                diagnostics.add(d);
                action.add("diagnostics", diagnostics);
                JsonObject edit = new JsonObject();
                JsonObject changes = new JsonObject();
                changes.add(uri, fix.get("edits"));
                edit.add("changes", changes);
                action.add("edit", edit);
                out.add(action);
            }
        }
        return out;
    }

    // ---- application properties and YAML

    private boolean propertiesMessage(String method, JsonObject message, JsonObject params, String uri) {
        String path = key(uri);
        JsonElement id = message.get("id");
        switch (method) {
            case "textDocument/didOpen" -> {
                uris.put(path, uri);
                documents.put(path, params.getAsJsonObject("textDocument").get("text").getAsString());
                validate(uri, 0);
            }
            case "textDocument/didChange" -> {
                String text = documents.getOrDefault(path, "");
                for (JsonElement c : params.getAsJsonArray("contentChanges")) {
                    text = applyChange(text, c.getAsJsonObject());
                }
                documents.put(path, text);
                validate(uri, 300);
            }
            case "textDocument/didClose" -> {
                documents.remove(path);
                springDiagnostics.remove(path);
                publish(path);
            }
            case "textDocument/didSave" -> {
                // The text is the one the app sent; the profiles it sets may have changed.
                pool.submit(this::refreshProfiles);
            }
            case "textDocument/completion", "textDocument/hover", "textDocument/definition" -> {
                if (id == null) {
                    return true;
                }
                String action = switch (method) {
                    case "textDocument/completion" -> "complete";
                    case "textDocument/hover" -> "hover";
                    default -> "definition";
                };
                pool.submit(() -> answer(id, action, uri, params));
            }
            default -> {
                if (id != null) {
                    // jdtls has nothing for these files: answer for it, emptily.
                    toApp.accept(Rpc.result(id, JsonNull.INSTANCE));
                }
            }
        }
        return true;
    }

    private void answer(JsonElement id, String action, String uri, JsonObject params) {
        try {
            scanned.get(30, TimeUnit.MINUTES);
            String text = documents.get(key(uri));
            if (!available || text == null) {
                toApp.accept(Rpc.result(id, JsonNull.INSTANCE));
                return;
            }
            JsonObject p = new JsonObject();
            p.addProperty("action", action);
            p.addProperty("uri", uri);
            p.addProperty("text", text);
            JsonObject position = params.getAsJsonObject("position");
            p.add("line", position.get("line"));
            p.add("character", position.get("character"));
            JsonElement result = command("snout.spring.properties", p, 120);
            toApp.accept(Rpc.result(id, result == null ? JsonNull.INSTANCE : result));
        } catch (Exception e) {
            toApp.accept(Rpc.error(id, -32603, "Spring properties " + action + " failed: " + SnoutEngine.describe(e)));
        }
    }

    private void validate(String uri, int delayMillis) {
        String path = key(uri);
        ScheduledFuture<?> previous = validations.get(path);
        if (previous != null) {
            previous.cancel(false);
        }
        validations.put(path, timer.schedule(() -> pool.submit(() -> {
            try {
                if (!scanned.isDone() || !available) {
                    return; // the scan validates open documents when it ends
                }
                String text = documents.get(path);
                if (text == null) {
                    return;
                }
                JsonObject p = new JsonObject();
                p.addProperty("action", "validate");
                p.addProperty("uri", uri);
                p.addProperty("text", text);
                JsonElement result = command("snout.spring.properties", p, 120);
                if (result != null && result.isJsonArray() && documents.containsKey(path)) {
                    springDiagnostics.put(path, result.getAsJsonArray());
                    publish(path);
                }
            } catch (Exception e) {
                log.println("snout-engine: validating " + uri + " failed: " + SnoutEngine.describe(e));
            }
        }), delayMillis, TimeUnit.MILLISECONDS));
    }

    /** Applies one LSP content change, ranged or whole. */
    static String applyChange(String text, JsonObject change) {
        String newText = change.get("text").getAsString();
        if (!change.has("range") || change.get("range").isJsonNull()) {
            return newText;
        }
        JsonObject range = change.getAsJsonObject("range");
        int start = offset(text, range.getAsJsonObject("start"));
        int end = offset(text, range.getAsJsonObject("end"));
        return text.substring(0, start) + newText + text.substring(Math.max(start, end));
    }

    private static int offset(String text, JsonObject position) {
        int line = position.get("line").getAsInt();
        int character = position.get("character").getAsInt();
        int offset = 0;
        for (int l = 0; l < line; l++) {
            int nl = text.indexOf('\n', offset);
            if (nl < 0) {
                return text.length();
            }
            offset = nl + 1;
        }
        int lineEnd = text.indexOf('\n', offset);
        if (lineEnd < 0) {
            lineEnd = text.length();
        }
        return Math.min(offset + character, lineEnd);
    }
}
