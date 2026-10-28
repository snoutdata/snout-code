package com.snoutcode.engine.lsp;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.google.gson.JsonPrimitive;
import org.junit.jupiter.api.AfterAll;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.MethodOrderer;
import org.junit.jupiter.api.Order;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.TestInstance;
import org.junit.jupiter.api.TestMethodOrder;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Duration;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * The engine's methods through {@code snout-engine lsp}, with jdtls and java-debug loaded:
 * Gradle tasks over the LSP stream, a main class resolved and run through jdtls's classpath,
 * and a debug session driven by a headless DAP client (breakpoint, stack, variables, step over,
 * evaluate, hot code replace, continue).
 */
@TestInstance(TestInstance.Lifecycle.PER_CLASS)
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
class LspRunDebugTest {
    private static final Duration LONG = Duration.ofMinutes(5);

    private Path project;
    private LspClient client;
    private JsonObject mainConfig;

    @BeforeAll
    void start() throws Exception {
        Path scratch = Path.of(System.getProperty("snout.test.scratch")).resolve("run-" + System.nanoTime());
        LspEndToEndTest.copy(Path.of(System.getProperty("snout.test.engineFixtures"), "run-app"), scratch.resolve("run-app"));
        project = scratch.resolve("run-app");
        List<String> command = List.of(System.getProperty("snout.test.engine"), "lsp", project.toString(),
            "--cache-dir", scratch.resolve("cache").toString(), "--gradle-version", System.getProperty("snout.test.gradleVersion"),
            "--java-home", System.getProperty("java.home"), "--no-sources");
        client = LspClient.start(command, Map.of("JAVA_HOME", System.getProperty("java.home")), scratch.resolve("stderr.log").toFile());
        Driver driver = new Driver(project, client);
        driver.initialize(new JsonObject());
        driver.awaitReady(Duration.ofMinutes(10));
        client.request("java/buildWorkspace", new JsonPrimitive(false), LONG);
    }

    @AfterAll
    void stop() throws IOException {
        if (client != null) {
            client.close();
        }
    }

    @Test
    @Order(1)
    void listsGradleTasksOverLsp() throws IOException {
        JsonObject result = client.request("snout/gradleTasks", new JsonObject(), LONG).getAsJsonObject();
        String text = result.toString();
        assertTrue(text.contains("\":app:hello\""), text);
        assertTrue(text.contains("Says hello."), text);
    }

    @Test
    @Order(2)
    void resolvesAMainClassThroughJdtls() throws IOException {
        JsonArray configs = client.request("snout/runConfigurations", new JsonObject(), LONG).getAsJsonObject()
            .getAsJsonArray("configurations");
        for (JsonElement c : configs) {
            if ("main".equals(c.getAsJsonObject().get("kind").getAsString())) {
                mainConfig = c.getAsJsonObject();
            }
        }
        assertEquals("demo.app.Main", mainConfig.get("mainClass").getAsString(), configs.toString());
        JsonObject params = new JsonObject();
        params.add("config", mainConfig);
        params.addProperty("dryRun", true);
        JsonObject resolved = client.request("snout/run", params, LONG).getAsJsonObject();
        List<String> classpath = new ArrayList<>();
        resolved.getAsJsonArray("classpath").forEach(e -> classpath.add(e.getAsString()));
        assertTrue(classpath.stream().anyMatch(p -> p.contains("workspace") && p.endsWith("bin/main")), classpath.toString());
        assertTrue(classpath.stream().anyMatch(p -> p.endsWith("app/src/main/resources")), classpath.toString());
        assertTrue(resolved.get("java").getAsString().endsWith("bin/java"), resolved.toString());
        assertFalse(resolved.get("projectName").isJsonNull(), resolved.toString());
    }

    @Test
    @Order(3)
    void runsAMainClass() throws Exception {
        JsonObject params = new JsonObject();
        params.add("config", mainConfig);
        String runId = client.request("snout/run", params, LONG).getAsJsonObject().get("runId").getAsString();
        JsonObject done = client.awaitNotification("snout/taskFinished", runId, p -> true, LONG);
        assertEquals(0, done.get("exitCode").getAsInt(), done + " " + client.outputLines(runId));
        List<String> lines = client.outputLines(runId);
        assertTrue(lines.contains("total 6"), lines.toString());
        assertTrue(lines.contains("hello debugger"), lines.toString());
        assertTrue(lines.contains("resource from resources"), lines.toString());
    }

    @Test
    @Order(4)
    void debugsAMainClass() throws Exception {
        JsonObject params = new JsonObject();
        params.add("config", mainConfig);
        params.addProperty("debug", true);
        JsonObject started = client.request("snout/run", params, LONG).getAsJsonObject();
        String runId = started.get("runId").getAsString();
        JsonObject debug = started.getAsJsonObject("debug");
        assertTrue(debug.get("listening").getAsBoolean(), started.toString());
        Path main = project.resolve("app/src/main/java/demo/app/Main.java");
        try (DapClient dap = DapClient.connect("127.0.0.1", debug.get("dapPort").getAsInt())) {
            dap.request("initialize", JsonParser.parseString("""
                { "clientID": "snout-test", "adapterID": "java", "linesStartAt1": true, "columnsStartAt1": true,
                  "pathFormat": "path", "supportsVariableType": true }"""), LONG);
            dap.request("attach", debug.getAsJsonObject("attach"), LONG);
            dap.awaitEvent("initialized", 0, b -> true, LONG);
            JsonObject breakpoints = dap.request("setBreakpoints", JsonParser.parseString("""
                { "source": { "path": "%s" }, "breakpoints": [ { "line": 14 } ] }""".formatted(LspEndToEndTest.json(main.toString()))), LONG);
            assertTrue(breakpoints.getAsJsonArray("breakpoints").get(0).getAsJsonObject().get("verified").getAsBoolean(), breakpoints.toString());
            dap.request("configurationDone", new JsonObject(), LONG);

            JsonObject stopped = dap.awaitEvent("stopped", 0, b -> "breakpoint".equals(b.get("reason").getAsString()), LONG);
            int thread = stopped.get("threadId").getAsInt();
            JsonObject frame = topFrame(dap, thread);
            assertEquals(14, frame.get("line").getAsInt(), frame.toString());
            assertTrue(frame.get("name").getAsString().contains("main"), frame.toString());
            Map<String, String> locals = locals(dap, frame.get("id").getAsInt());
            assertEquals("1", locals.get("i"), locals.toString());
            assertEquals("0", locals.get("total"), locals.toString());

            int events = dap.events.size();
            dap.request("next", JsonParser.parseString("{ \"threadId\": " + thread + " }"), LONG);
            dap.awaitEvent("stopped", events, b -> "step".equals(b.get("reason").getAsString()), LONG);
            JsonObject stepped = topFrame(dap, thread);
            assertNotEquals(14, stepped.get("line").getAsInt(), stepped.toString());
            JsonObject value = dap.request("evaluate", JsonParser.parseString("""
                { "expression": "total + 100", "frameId": %d, "context": "repl" }""".formatted(stepped.get("id").getAsInt())), LONG);
            assertEquals("101", value.get("result").getAsString(), value.toString());

            // Hot code replace: add() changes while the program is stopped, and the loop's
            // remaining calls run the new code.
            // As an editor does it: open, change, save; jdtls builds, java-debug announces the
            // changed classes, and the client asks for them to be redefined.
            String edited = Files.readString(main).replace("return a + b;", "return a + b * 10;");
            client.didOpen(main);
            JsonObject change = new JsonObject();
            JsonObject document = new JsonObject();
            document.addProperty("uri", LspClient.uri(main));
            document.addProperty("version", 2);
            change.add("textDocument", document);
            JsonArray changes = new JsonArray();
            JsonObject whole = new JsonObject();
            whole.addProperty("text", edited);
            changes.add(whole);
            change.add("contentChanges", changes);
            int before = dap.events.size();
            client.notify("textDocument/didChange", change);
            Files.writeString(main, edited);
            JsonObject saved = new JsonObject();
            JsonObject savedDocument = new JsonObject();
            savedDocument.addProperty("uri", LspClient.uri(main));
            saved.add("textDocument", savedDocument);
            client.notify("textDocument/didSave", saved);
            dap.awaitEvent("hotcodereplace", before, b -> "BUILD_COMPLETE".equals(String.valueOf(b.get("changeType")).replace("\"", "")), LONG);
            JsonObject redefined = dap.send("redefineClasses", new JsonObject(), LONG);
            assertTrue(redefined.get("success").getAsBoolean(), redefined.toString());
            assertTrue(redefined.toString().contains("demo.app.Main"), redefined.toString());

            dap.request("setBreakpoints", JsonParser.parseString("""
                { "source": { "path": "%s" }, "breakpoints": [] }""".formatted(LspEndToEndTest.json(main.toString()))), LONG);
            dap.request("continue", JsonParser.parseString("{ \"threadId\": " + thread + " }"), LONG);
            JsonObject done = client.awaitNotification("snout/taskFinished", runId, p -> true, LONG);
            assertEquals(0, done.get("exitCode").getAsInt(), done.toString());
        }
        List<String> lines = client.outputLines(runId);
        // 1 + 2 * 10 + 3 * 10: the first call ran the old add, the next two the new one.
        assertTrue(lines.contains("total 51"), lines.toString());
    }

    @Test
    @Order(5)
    void debugsAGradleRunTask() throws Exception {
        JsonArray configs = client.request("snout/runConfigurations", new JsonObject(), LONG).getAsJsonObject()
            .getAsJsonArray("configurations");
        JsonObject run = null;
        for (JsonElement c : configs) {
            if ("gradle".equals(c.getAsJsonObject().get("kind").getAsString())) {
                run = c.getAsJsonObject();
            }
        }
        assertEquals(":app:run", run.get("task").getAsString(), configs.toString());
        JsonObject params = new JsonObject();
        params.add("config", run);
        params.addProperty("debug", true);
        JsonObject started = client.request("snout/run", params, LONG).getAsJsonObject();
        String runId = started.get("runId").getAsString();
        JsonObject ready = client.awaitNotification("snout/debugReady", runId, p -> true, LONG);
        try (DapClient dap = DapClient.connect("127.0.0.1", ready.get("dapPort").getAsInt())) {
            dap.request("initialize", JsonParser.parseString("{ \"adapterID\": \"java\", \"linesStartAt1\": true, \"pathFormat\": \"path\" }"), LONG);
            dap.request("attach", ready.getAsJsonObject("attach"), LONG);
            dap.awaitEvent("initialized", 0, b -> true, LONG);
            dap.request("configurationDone", new JsonObject(), LONG);
            JsonObject done = client.awaitNotification("snout/taskFinished", runId, p -> true, LONG);
            assertEquals("success", done.get("outcome").getAsString(), done + " " + client.outputLines(runId));
        }
        assertTrue(client.outputLines(runId).stream().anyMatch(l -> l.startsWith("total ")), client.outputLines(runId).toString());
    }

    private static JsonObject topFrame(DapClient dap, int thread) throws IOException {
        JsonObject trace = dap.request("stackTrace", JsonParser.parseString("{ \"threadId\": " + thread + ", \"levels\": 20 }"), LONG);
        return trace.getAsJsonArray("stackFrames").get(0).getAsJsonObject();
    }

    private static Map<String, String> locals(DapClient dap, int frameId) throws IOException {
        JsonObject scopes = dap.request("scopes", JsonParser.parseString("{ \"frameId\": " + frameId + " }"), LONG);
        int reference = scopes.getAsJsonArray("scopes").get(0).getAsJsonObject().get("variablesReference").getAsInt();
        JsonObject variables = dap.request("variables", JsonParser.parseString("{ \"variablesReference\": " + reference + " }"), LONG);
        Map<String, String> out = new java.util.TreeMap<>();
        for (JsonElement v : variables.getAsJsonArray("variables")) {
            out.put(v.getAsJsonObject().get("name").getAsString(), v.getAsJsonObject().get("value").getAsString());
        }
        return out;
    }
}
