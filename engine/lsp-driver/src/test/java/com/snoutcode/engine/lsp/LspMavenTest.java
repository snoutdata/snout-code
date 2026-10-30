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
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * A Maven reactor through {@code snout-engine lsp}: imported into jdtls from the engine's own
 * model (m2e does not import it), the Maven window's goals, a main class run as jdtls compiled
 * it, a module's tests through surefire with their events, and a test debugged through
 * java-debug. Maven is the one the engine downloads; the local repository is the test's.
 */
@TestInstance(TestInstance.Lifecycle.PER_CLASS)
@TestMethodOrder(MethodOrderer.OrderAnnotation.class)
class LspMavenTest {
    private static final Duration LONG = Duration.ofMinutes(10);

    private Path project;
    private LspClient client;

    @BeforeAll
    void start() throws Exception {
        Path scratch = Path.of(System.getProperty("snout.test.scratch")).resolve("maven-" + System.nanoTime());
        LspEndToEndTest.copy(Path.of(System.getProperty("snout.test.engineFixtures"), "maven-multi"), scratch.resolve("maven-multi"));
        project = scratch.resolve("maven-multi");
        // The Maven the engine downloads, shared with the other tests' cache.
        Path shared = Path.of(System.getProperty("snout.test.mavenCache"), "maven");
        Files.createDirectories(shared);
        Files.createDirectories(scratch.resolve("cache"));
        Files.createSymbolicLink(scratch.resolve("cache/maven"), shared);
        List<String> command = List.of(System.getProperty("snout.test.engine"), "lsp", project.toString(),
            "--cache-dir", scratch.resolve("cache").toString(), "--java-home", System.getProperty("java.home"), "--no-sources");
        String opts = "-Dmaven.repo.local=" + System.getProperty("snout.test.m2");
        client = LspClient.start(command, Map.of("JAVA_HOME", System.getProperty("java.home"), "MAVEN_OPTS", opts),
            scratch.resolve("stderr.log").toFile());
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
    void importsTheReactorWithoutErrors() throws Exception {
        Path main = project.resolve("app/src/main/java/demo/app/Main.java");
        Path greeter = project.resolve("core/src/main/java/demo/core/Greeter.java");
        // The test uses the library's tests, through its test jar.
        Path test = project.resolve("app/src/test/java/demo/app/MainTest.java");
        client.didOpen(main);
        client.didOpen(greeter);
        client.didOpen(test);
        client.awaitDiagnosticsFor(List.of(main, greeter, test), Duration.ofMinutes(2));
        client.awaitDiagnosticsQuiet(Duration.ofSeconds(3), Duration.ofMinutes(2));
        for (Path f : List.of(main, greeter, test)) {
            JsonArray d = client.diagnostics.get(LspClient.uri(f));
            long errors = d == null ? 0 : d.asList().stream().filter(x -> x.getAsJsonObject().get("severity").getAsInt() == 1).count();
            assertEquals(0, errors, f + ": " + d);
        }
        // m2e left the folder alone: no Eclipse metadata in the project.
        assertTrue(Files.notExists(project.resolve(".project")) && Files.notExists(project.resolve("app/.classpath")));
    }

    @Test
    @Order(2)
    void listsGoalsAndProfiles() throws IOException {
        JsonObject result = client.request("snout/mavenGoals", new JsonObject(), LONG).getAsJsonObject();
        String text = result.toString();
        assertTrue(text.contains("\"module\":\"maven-multi:app\""), text);
        assertTrue(text.contains("\"prefix\":\"surefire\""), text);
        assertTrue(text.contains("\"name\":\"test\""), text);
        assertTrue(text.contains("\"id\":\"extra\""), text);
    }

    @Test
    @Order(3)
    void runsAMainClassAsJdtlsCompiledIt() throws Exception {
        JsonObject config = null;
        JsonArray configs = client.request("snout/runConfigurations", new JsonObject(), LONG).getAsJsonObject().getAsJsonArray("configurations");
        for (JsonElement c : configs) {
            if ("demo.app.Main".equals(LspEndToEndTestSupport.string(c.getAsJsonObject(), "mainClass"))) {
                config = c.getAsJsonObject();
            }
        }
        assertEquals("maven-multi:app", config.get("module").getAsString(), configs.toString());
        JsonObject params = new JsonObject();
        params.add("config", config);
        String runId = client.request("snout/run", params, LONG).getAsJsonObject().get("runId").getAsString();
        JsonObject done = client.awaitNotification("snout/taskFinished", runId, p -> true, LONG);
        List<String> lines = client.outputLines(runId);
        assertEquals(0, done.get("exitCode").getAsInt(), lines.toString());
        assertTrue(lines.contains("total 6"), lines.toString());
        assertTrue(lines.contains("Hello, Maven!"), lines.toString());
        assertTrue(lines.contains("resource from resources"), lines.toString());
    }

    @Test
    @Order(4)
    void runsAClassOfTestsThroughSurefire() throws Exception {
        JsonObject params = JsonParser.parseString("""
            { "module": "maven-multi:app", "filter": { "kind": "method", "class": "demo.app.MainTest", "method": "adds" } }""")
            .getAsJsonObject();
        String runId = client.request("snout/runTests", params, LONG).getAsJsonObject().get("runId").getAsString();
        JsonObject done = client.awaitNotification("snout/taskFinished", runId, p -> true, LONG);
        assertEquals("success", done.get("outcome").getAsString(), client.outputLines(runId).toString());
        assertEquals(1, done.getAsJsonObject("tests").get("passed").getAsInt(), done.toString());
        List<String> names = new ArrayList<>();
        client.notifications("snout/testEvent", runId).forEach(e -> names.add(e.get("name").getAsString()));
        assertTrue(names.contains("adds()"), names.toString());
    }

    @Test
    @Order(5)
    void debugsTheMainClass() throws Exception {
        JsonObject config = null;
        JsonArray configs = client.request("snout/runConfigurations", new JsonObject(), LONG).getAsJsonObject().getAsJsonArray("configurations");
        for (JsonElement c : configs) {
            if ("demo.app.Main".equals(LspEndToEndTestSupport.string(c.getAsJsonObject(), "mainClass"))) {
                config = c.getAsJsonObject();
            }
        }
        JsonObject params = new JsonObject();
        params.add("config", config);
        params.addProperty("debug", true);
        JsonObject started = client.request("snout/run", params, LONG).getAsJsonObject();
        JsonObject debug = started.getAsJsonObject("debug");
        Path main = project.resolve("app/src/main/java/demo/app/Main.java");
        try (DapClient dap = DapClient.connect("127.0.0.1", debug.get("dapPort").getAsInt())) {
            dap.request("initialize", JsonParser.parseString("{ \"adapterID\": \"java\", \"linesStartAt1\": true, \"columnsStartAt1\": true, \"pathFormat\": \"path\" }"), LONG);
            dap.request("attach", debug.getAsJsonObject("attach"), LONG);
            dap.awaitEvent("initialized", 0, b -> true, LONG);
            JsonObject breakpoints = dap.request("setBreakpoints", JsonParser.parseString("""
                { "source": { "path": "%s" }, "breakpoints": [ { "line": 22 } ] }""".formatted(LspEndToEndTest.json(main.toString()))), LONG);
            dap.request("configurationDone", new JsonObject(), LONG);
            JsonObject stopped;
            try {
                stopped = dap.awaitEvent("stopped", 0, b -> "breakpoint".equals(b.get("reason").getAsString()), Duration.ofMinutes(1));
            } catch (IOException e) {
                throw new AssertionError(breakpoints + " " + debug, e);
            }
            dap.request("continue", JsonParser.parseString("{ \"threadId\": " + stopped.get("threadId").getAsInt() + " }"), LONG);
        }
    }

    @Test
    @Order(6)
    void debugsATest() throws Exception {
        JsonObject params = JsonParser.parseString("""
            { "module": "maven-multi:app", "debug": true,
              "filter": { "kind": "method", "class": "demo.app.MainTest", "method": "adds" } }""").getAsJsonObject();
        JsonObject started = client.request("snout/runTests", params, LONG).getAsJsonObject();
        String runId = started.get("runId").getAsString();
        JsonObject ready = client.awaitNotification("snout/debugReady", runId, p -> true, LONG);
        Path test = project.resolve("app/src/test/java/demo/app/MainTest.java");
        try (DapClient dap = DapClient.connect("127.0.0.1", ready.get("dapPort").getAsInt())) {
            dap.request("initialize", JsonParser.parseString("{ \"adapterID\": \"java\", \"linesStartAt1\": true, \"columnsStartAt1\": true, \"pathFormat\": \"path\" }"), LONG);
            dap.request("attach", ready.getAsJsonObject("attach"), LONG);
            dap.awaitEvent("initialized", 0, b -> true, LONG);
            JsonObject breakpoints = dap.request("setBreakpoints", JsonParser.parseString("""
                { "source": { "path": "%s" }, "breakpoints": [ { "line": 14 } ] }""".formatted(LspEndToEndTest.json(test.toString()))), LONG);
            dap.request("configurationDone", new JsonObject(), LONG);
            JsonObject stopped;
            try {
                stopped = dap.awaitEvent("stopped", 0, b -> "breakpoint".equals(b.get("reason").getAsString()), Duration.ofMinutes(2));
            } catch (IOException e) {
                throw new AssertionError(breakpoints + "\n" + String.join("\n", client.outputLines(runId)), e);
            }
            int thread = stopped.get("threadId").getAsInt();
            JsonObject trace = dap.request("stackTrace", JsonParser.parseString("{ \"threadId\": " + thread + ", \"levels\": 5 }"), LONG);
            JsonObject frame = trace.getAsJsonArray("stackFrames").get(0).getAsJsonObject();
            assertEquals(14, frame.get("line").getAsInt(), frame.toString());
            dap.request("continue", JsonParser.parseString("{ \"threadId\": " + thread + " }"), LONG);
            JsonObject done = client.awaitNotification("snout/taskFinished", runId, p -> true, LONG);
            assertEquals("success", done.get("outcome").getAsString(), client.outputLines(runId).toString());
        }
    }

    /** Small JSON helpers. */
    static final class LspEndToEndTestSupport {
        static String string(JsonObject o, String key) {
            return o.has(key) && !o.get(key).isJsonNull() ? o.get(key).getAsString() : null;
        }
    }
}
