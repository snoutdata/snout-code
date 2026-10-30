package com.snoutcode.engine.lsp;

import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.google.gson.JsonPrimitive;

import java.io.IOException;
import java.net.URI;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Duration;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TreeMap;

/**
 * Drives {@code snout-engine lsp} headlessly: starts it, waits for the import and the first
 * build, opens files, collects diagnostics and runs the navigation checks of a script.
 *
 * <pre>
 * lsp-driver --engine &lt;bin/snout-engine&gt; --project &lt;dir&gt; --script &lt;script.json&gt; [--report &lt;out.json&gt;]
 * </pre>
 *
 * The script is JSON:
 *
 * <pre>
 * {
 *   "engineArgs": ["--java-home", "/path/to/jdk17"],
 *   "env": { "JAVA_HOME": "/path/to/jdk21" },
 *   "open": ["src/main/java/demo/App.java"],
 *   "checks": [
 *     { "name": "...", "kind": "definition" | "completion" | "references",
 *       "file": "src/main/java/demo/App.java", "find": "Lib.hello", "occurrence": 1, "offset": 4,
 *       "expect": "substring of a result URI or completion label" },
 *     { "kind": "runTests", "module": "shop:app", "filter": { "kind": "all" } },
 *     { "kind": "run", "mainClass": "demo.App", "expect": "a line of its output" }
 *   ]
 * }
 * </pre>
 *
 * {@code capabilities} replaces the client capabilities sent in {@code initialize}, and
 * {@code settings} goes in its initialization options, to start jdtls as a given client does.
 * The report has the milliseconds from the engine's start to jdtls's {@code ServiceReady}, to
 * every module ready, and to the first diagnostics of each file opened.
 *
 * A definition check with {@code "single": true} passes only with exactly one result. A references
 * check passes with at least {@code min} results, at most {@code max} when given, and with
 * {@code "distinct": true} only when no location is listed twice; {@code "listAll": true} puts
 * every location in the report.
 *
 * Files are relative to the project directory unless absolute. A position is the start of the
 * {@code occurrence}-th match of {@code find} in the file, plus {@code offset} characters.
 */
public final class Driver {
    private final Path projectDir;
    private final LspClient client;

    public Driver(Path projectDir, LspClient client) {
        this.projectDir = projectDir;
        this.client = client;
    }

    public static void main(String[] args) throws Exception {
        Map<String, String> options = new HashMap<>();
        for (int i = 0; i + 1 < args.length; i += 2) {
            options.put(args[i], args[i + 1]);
        }
        if (!options.containsKey("--engine") || !options.containsKey("--project") || !options.containsKey("--script")) {
            System.err.println("usage: lsp-driver --engine <bin/snout-engine> --project <dir> --script <script.json> [--report <out.json>]");
            System.exit(2);
        }
        Path project = Path.of(options.get("--project")).toAbsolutePath().normalize();
        JsonObject script = JsonParser.parseString(Files.readString(Path.of(options.get("--script")))).getAsJsonObject();
        JsonObject report = run(Path.of(options.get("--engine")), project, script,
            Path.of(options.getOrDefault("--stderr", "lsp-driver-stderr.log")));
        String json = new GsonBuilder().setPrettyPrinting().disableHtmlEscaping().create().toJson(report);
        if (options.containsKey("--report")) {
            Files.writeString(Path.of(options.get("--report")), json + "\n");
        }
        System.out.println(json);
    }

    /** Starts the engine, runs the script and returns the report. */
    public static JsonObject run(Path engine, Path project, JsonObject script, Path stderrLog) throws Exception {
        List<String> command = new ArrayList<>();
        command.add(engine.toString());
        command.add("lsp");
        command.add(project.toString());
        for (JsonElement a : array(script, "engineArgs")) {
            command.add(a.getAsString());
        }
        Map<String, String> env = new HashMap<>();
        if (script.has("env")) {
            script.getAsJsonObject("env").entrySet().forEach(e -> env.put(e.getKey(), e.getValue().getAsString()));
        }
        JsonObject report = new JsonObject();
        try (LspClient client = LspClient.start(command, env, stderrLog.toFile())) {
            Driver driver = new Driver(project, client);
            driver.initialize(script);
            List<Path> opened = new ArrayList<>();
            for (JsonElement f : array(script, "open")) {
                Path file = driver.resolve(f.getAsString());
                client.didOpen(file);
                opened.add(file);
            }
            long ready = driver.awaitReady(Duration.ofMinutes(20));
            report.addProperty("serviceReadyMillis", ready);
            report.addProperty("projectReadyMillis", driver.awaitProjectReady(Duration.ofMinutes(20)));
            long buildStart = System.nanoTime();
            JsonElement build = client.request("java/buildWorkspace", new JsonPrimitive(false), Duration.ofMinutes(30));
            report.addProperty("buildStatus", build.toString());
            client.awaitDiagnosticsFor(opened, Duration.ofMinutes(2));
            client.awaitDiagnosticsQuiet(Duration.ofSeconds(5), Duration.ofMinutes(5));
            report.addProperty("buildMillis", (System.nanoTime() - buildStart) / 1_000_000);
            report.add("firstDiagnosticsMillis", driver.firstDiagnostics(opened));
            report.add("diagnostics", driver.summarizeDiagnostics());
            JsonArray checks = new JsonArray();
            for (JsonElement c : array(script, "checks")) {
                try {
                    checks.add(driver.check(c.getAsJsonObject()));
                } catch (IOException | RuntimeException e) {
                    JsonObject failed = new JsonObject();
                    failed.add("check", c);
                    failed.addProperty("error", String.valueOf(e.getMessage()));
                    failed.addProperty("pass", false);
                    checks.add(failed);
                }
            }
            report.add("checks", checks);
            report.addProperty("peakJdtlsRssMb", client.peakJdtlsRssKb() / 1024);
            report.add("statuses", new GsonBuilder().create().toJsonTree(client.statuses));
        }
        return report;
    }

    // ---- session

    public void initialize(JsonObject script) throws IOException {
        JsonObject params = new JsonObject();
        params.addProperty("processId", ProcessHandle.current().pid());
        params.addProperty("rootUri", LspClient.uri(projectDir));
        params.addProperty("rootPath", projectDir.toString());
        JsonArray folders = new JsonArray();
        JsonObject folder = new JsonObject();
        folder.addProperty("uri", LspClient.uri(projectDir));
        folder.addProperty("name", projectDir.getFileName().toString());
        folders.add(folder);
        params.add("workspaceFolders", folders);
        params.add("capabilities", script.has("capabilities") ? script.get("capabilities") : JsonParser.parseString("""
            {
              "workspace": { "configuration": true, "workspaceFolders": true,
                             "didChangeWatchedFiles": { "dynamicRegistration": true } },
              "textDocument": {
                "synchronization": { "didSave": true },
                "publishDiagnostics": { "relatedInformation": false },
                "completion": { "completionItem": { "snippetSupport": false } },
                "definition": { "linkSupport": false },
                "references": {}
              }
            }"""));
        JsonObject init = new JsonObject();
        init.add("settings", script.has("settings") ? script.get("settings") : new JsonObject());
        init.add("extendedClientCapabilities", JsonParser.parseString(
            "{ \"classFileContentsSupport\": true, \"progressReportProvider\": false }"));
        params.add("initializationOptions", init);
        client.request("initialize", params, Duration.ofMinutes(20));
        client.notify("initialized", new JsonObject());
    }

    /** Waits for jdtls's ServiceReady; returns the milliseconds since the engine started. */
    public long awaitReady(Duration timeout) throws Exception {
        client.serviceReady.get(timeout.toMillis(), java.util.concurrent.TimeUnit.MILLISECONDS);
        return (System.nanoTime() - client.startNanos) / 1_000_000;
    }

    /**
     * Waits until the engine says every module is ready ({@code snout/readiness} for the whole
     * project at {@code ready}); returns the milliseconds since the engine started, or -1 when the
     * engine sends no readiness.
     */
    public long awaitProjectReady(Duration timeout) throws InterruptedException {
        long deadline = System.nanoTime() + timeout.toNanos();
        while (System.nanoTime() < deadline) {
            boolean any = false;
            for (JsonObject n : client.snoutNotifications) {
                if (!"snout/readiness".equals(n.get("method").getAsString())) {
                    continue;
                }
                any = true;
                JsonObject p = n.getAsJsonObject("params");
                if (p.get("module").isJsonNull() && "ready".equals(p.get("stage").getAsString())) {
                    return (System.nanoTime() - client.startNanos) / 1_000_000;
                }
            }
            if (!any) {
                return -1; // an engine that reports no readiness: ready with jdtls
            }
            Thread.sleep(100);
        }
        return -1;
    }

    public Path resolve(String file) {
        Path p = Path.of(file);
        return p.isAbsolute() ? p : projectDir.resolve(p);
    }

    private JsonObject firstDiagnostics(List<Path> opened) {
        JsonObject out = new JsonObject();
        for (Path file : opened) {
            Long nanos = client.firstDiagnostics.get(LspClient.uri(file));
            out.addProperty(projectDir.relativize(file).toString(), nanos == null ? null : nanos / 1_000_000);
        }
        return out;
    }

    // ---- diagnostics

    /** Errors and warnings across all Java files, and the kinds of errors with a sample each. */
    public JsonObject summarizeDiagnostics() {
        int errors = 0;
        int warnings = 0;
        int filesWithErrors = 0;
        int javaFiles = 0;
        Map<String, Integer> kinds = new TreeMap<>();
        Map<String, String> samples = new HashMap<>();
        JsonArray other = new JsonArray();
        for (Map.Entry<String, JsonArray> e : client.diagnostics.entrySet()) {
            boolean java = e.getKey().endsWith(".java");
            int fileErrors = 0;
            for (JsonElement d : e.getValue()) {
                JsonObject diagnostic = d.getAsJsonObject();
                int severity = diagnostic.has("severity") ? diagnostic.get("severity").getAsInt() : 1;
                if (!java) {
                    if (severity == 1) {
                        other.add(e.getKey() + ": " + diagnostic.get("message").getAsString());
                    }
                    continue;
                }
                if (severity == 1) {
                    fileErrors++;
                    String code = diagnostic.has("code") ? diagnostic.get("code").getAsString() : "?";
                    String kind = code + " " + kindOf(diagnostic.get("message").getAsString());
                    kinds.merge(kind, 1, Integer::sum);
                    samples.putIfAbsent(kind, shortUri(e.getKey()) + ": " + diagnostic.get("message").getAsString());
                } else if (severity == 2) {
                    warnings++;
                }
            }
            if (java) {
                javaFiles++;
                errors += fileErrors;
                if (fileErrors > 0) {
                    filesWithErrors++;
                }
            }
        }
        JsonObject out = new JsonObject();
        out.addProperty("javaFilesReported", javaFiles);
        out.addProperty("errors", errors);
        out.addProperty("filesWithErrors", filesWithErrors);
        out.addProperty("warnings", warnings);
        JsonArray kindList = new JsonArray();
        kinds.entrySet().stream()
            .sorted((a, b) -> b.getValue() - a.getValue())
            .forEach(k -> {
                JsonObject kind = new JsonObject();
                kind.addProperty("kind", k.getKey());
                kind.addProperty("count", k.getValue());
                kind.addProperty("sample", samples.get(k.getKey()));
                kindList.add(kind);
            });
        out.add("errorKinds", kindList);
        out.add("nonJavaErrors", other);
        return out;
    }

    /** A message with its names taken out, so that one kind of error groups together. */
    static String kindOf(String message) {
        return message
            .replaceAll("'[^']*'", "'_'")
            .replaceAll("\\b[A-Za-z_$][\\w$]*(\\.[A-Za-z_$][\\w$]*)+\\b", "_")
            .replaceAll("^[A-Za-z_$][\\w$<>\\[\\], ?]* (cannot be resolved)", "_ $1")
            .replaceAll("\\([^)]*\\)", "(_)")
            .replaceAll("\\b[A-Z][\\w$]*(<[^>]*>)?\\b", "_");
    }

    private String shortUri(String uri) {
        try {
            int hash = uri.indexOf('#');
            Path p = Path.of(URI.create(hash < 0 ? uri : uri.substring(0, hash)));
            Path parent = projectDir.getParent();
            String suffix = hash < 0 ? "" : uri.substring(hash);
            return (parent != null && p.startsWith(parent) ? parent.relativize(p).toString() : p.toString()) + suffix;
        } catch (RuntimeException e) {
            return uri;
        }
    }

    // ---- checks

    public JsonObject check(JsonObject check) throws IOException {
        String kind = check.get("kind").getAsString();
        if (kind.equals("runTests") || kind.equals("run")) {
            return runCheck(kind, check);
        }
        Path file = resolve(check.get("file").getAsString());
        int[] pos = find(file, check.get("find").getAsString(),
            check.has("occurrence") ? check.get("occurrence").getAsInt() : 1,
            check.has("offset") ? check.get("offset").getAsInt() : 0);
        JsonObject params = LspClient.position(file, pos[0], pos[1]);
        JsonObject out = new JsonObject();
        out.addProperty("name", check.has("name") ? check.get("name").getAsString() : kind);
        out.addProperty("at", projectDir.relativize(file) + ":" + (pos[0] + 1) + ":" + (pos[1] + 1));
        String expect = check.has("expect") ? check.get("expect").getAsString() : null;
        long start = System.nanoTime();
        boolean pass;
        switch (kind) {
            case "definition" -> {
                JsonElement result = client.request("textDocument/definition", params, Duration.ofMinutes(2));
                List<String> uris = locations(result);
                out.add("results", new GsonBuilder().create().toJsonTree(uris));
                pass = !uris.isEmpty() && (expect == null || uris.stream().anyMatch(u -> u.contains(expect)))
                    && (!flag(check, "single") || uris.size() == 1);
                if (!uris.isEmpty() && uris.get(0).startsWith("jdt://")) {
                    JsonObject uri = new JsonObject();
                    uri.addProperty("uri", uris.get(0).replaceAll("#.*", ""));
                    JsonElement contents = client.request("java/classFileContents", uri, Duration.ofMinutes(2));
                    String text = contents.isJsonNull() ? "" : contents.getAsString();
                    out.addProperty("classFileSource", text.contains("Fernflower") || text.contains("decompiled")
                        ? "decompiled" : text.isEmpty() ? "none" : "attached source");
                }
            }
            case "completion" -> {
                JsonElement result = client.request("textDocument/completion", params, Duration.ofMinutes(2));
                JsonArray items = result.isJsonArray() ? result.getAsJsonArray()
                    : result.isJsonObject() ? result.getAsJsonObject().getAsJsonArray("items") : new JsonArray();
                List<String> labels = new ArrayList<>();
                for (JsonElement i : items) {
                    labels.add(i.getAsJsonObject().get("label").getAsString());
                }
                out.addProperty("count", labels.size());
                out.add("first", new GsonBuilder().create().toJsonTree(labels.subList(0, Math.min(10, labels.size()))));
                pass = !labels.isEmpty() && (expect == null || labels.stream().anyMatch(l -> l.contains(expect)));
            }
            case "references" -> {
                JsonObject context = new JsonObject();
                context.addProperty("includeDeclaration", true);
                params.add("context", context);
                JsonElement result = client.request("textDocument/references", params, Duration.ofMinutes(5));
                List<String> uris = locations(result);
                Set<String> distinct = new LinkedHashSet<>(uris);
                Set<String> files = new LinkedHashSet<>();
                uris.forEach(u -> files.add(u.replaceAll("#.*", "")));
                out.addProperty("count", uris.size());
                out.addProperty("distinct", distinct.size());
                out.addProperty("files", files.size());
                List<String> sample = new ArrayList<>();
                for (String u : files) {
                    if (sample.size() < 8) {
                        sample.add(shortUri(u));
                    }
                }
                out.add("sample", new GsonBuilder().create().toJsonTree(sample));
                if (flag(check, "listAll")) {
                    List<String> all = new ArrayList<>();
                    uris.forEach(u -> all.add(shortUri(u)));
                    out.add("all", new GsonBuilder().create().toJsonTree(all));
                }
                int min = check.has("min") ? check.get("min").getAsInt() : 1;
                int max = check.has("max") ? check.get("max").getAsInt() : Integer.MAX_VALUE;
                pass = uris.size() >= min && uris.size() <= max && (!flag(check, "distinct") || distinct.size() == uris.size());
            }
            default -> throw new IllegalArgumentException("unknown check kind " + kind);
        }
        out.addProperty("millis", (System.nanoTime() - start) / 1_000_000);
        out.addProperty("pass", pass);
        return out;
    }

    /**
     * A run through the engine: {@code runTests} runs a module's tests ({@code module}, and
     * {@code filter} as {@code snout/runTests} takes it) and passes when the run's outcome is
     * {@code expect} (default {@code success}) and at least one test passed; {@code run} launches
     * the run configuration of {@code mainClass} and passes when it exits with 0 and its output
     * has a line containing {@code expect}.
     */
    private JsonObject runCheck(String kind, JsonObject check) throws IOException {
        JsonObject out = new JsonObject();
        out.addProperty("name", check.has("name") ? check.get("name").getAsString() : kind);
        long start = System.nanoTime();
        Duration limit = Duration.ofMinutes(10);
        String runId;
        if (kind.equals("runTests")) {
            JsonObject params = new JsonObject();
            params.add("module", check.get("module"));
            if (check.has("filter")) {
                params.add("filter", check.get("filter"));
            }
            runId = client.request("snout/runTests", params, limit).getAsJsonObject().get("runId").getAsString();
        } else {
            JsonObject config = null;
            JsonArray configs = client.request("snout/runConfigurations", new JsonObject(), limit).getAsJsonObject()
                .getAsJsonArray("configurations");
            for (JsonElement c : configs) {
                JsonObject o = c.getAsJsonObject();
                if (o.has("mainClass") && o.get("mainClass").getAsString().equals(check.get("mainClass").getAsString())) {
                    config = o;
                }
            }
            if (config == null) {
                throw new IOException("no run configuration of " + check.get("mainClass") + " in " + configs);
            }
            JsonObject params = new JsonObject();
            params.add("config", config);
            runId = client.request("snout/run", params, limit).getAsJsonObject().get("runId").getAsString();
        }
        JsonObject done;
        try {
            done = client.awaitNotification("snout/taskFinished", runId, p -> true, limit);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            throw new IOException(e);
        }
        List<String> lines = client.outputLines(runId);
        out.add("finished", done);
        boolean pass;
        if (kind.equals("runTests")) {
            String expect = check.has("expect") ? check.get("expect").getAsString() : "success";
            int passed = done.has("tests") ? done.getAsJsonObject("tests").get("passed").getAsInt() : 0;
            pass = expect.equals(done.get("outcome").getAsString()) && passed > 0;
        } else {
            String expect = check.get("expect").getAsString();
            pass = done.has("exitCode") && done.get("exitCode").getAsInt() == 0 && lines.stream().anyMatch(l -> l.contains(expect));
        }
        if (!pass) {
            out.add("output", new GsonBuilder().create().toJsonTree(lines.subList(Math.max(0, lines.size() - 40), lines.size())));
        }
        out.addProperty("millis", (System.nanoTime() - start) / 1_000_000);
        out.addProperty("pass", pass);
        return out;
    }

    private static boolean flag(JsonObject check, String name) {
        return check.has(name) && check.get(name).getAsBoolean();
    }

    private static List<String> locations(JsonElement result) {
        List<String> out = new ArrayList<>();
        if (result == null || result.isJsonNull()) {
            return out;
        }
        JsonArray array = result.isJsonArray() ? result.getAsJsonArray() : new JsonArray();
        if (result.isJsonObject()) {
            array.add(result);
        }
        for (JsonElement e : array) {
            JsonObject location = e.getAsJsonObject();
            String uri = location.has("uri") ? location.get("uri").getAsString() : location.get("targetUri").getAsString();
            JsonObject range = location.has("range") ? location.getAsJsonObject("range") : location.getAsJsonObject("targetRange");
            JsonObject start = range.getAsJsonObject("start");
            out.add(uri + "#" + (start.get("line").getAsInt() + 1) + ":" + (start.get("character").getAsInt() + 1));
        }
        return out;
    }

    /** Line and character (UTF-16, as LSP counts) of the n-th match of {@code text}, plus offset. */
    static int[] find(Path file, String text, int occurrence, int offset) throws IOException {
        String content = Files.readString(file);
        int index = -1;
        for (int i = 0; i < occurrence; i++) {
            index = content.indexOf(text, index + 1);
            if (index < 0) {
                throw new IllegalArgumentException("'" + text + "' (" + occurrence + ") not found in " + file);
            }
        }
        index += offset;
        int line = 0;
        int lineStart = 0;
        for (int i = 0; i < index; i++) {
            if (content.charAt(i) == '\n') {
                line++;
                lineStart = i + 1;
            }
        }
        return new int[] {line, index - lineStart};
    }

    private static JsonArray array(JsonObject o, String key) {
        return o.has(key) && o.get(key).isJsonArray() ? o.getAsJsonArray(key) : new JsonArray();
    }
}
