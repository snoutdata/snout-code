package com.snoutcode.engine.cli;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.AfterAll;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.TestInstance;

import java.io.IOException;
import java.io.OutputStream;
import java.io.PrintStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Duration;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.function.Predicate;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * The engine's Gradle methods on a real build, without jdtls: listing tasks, running one with
 * its output and progress, compiler problems, cancelling, tests with their events and filters,
 * rerunning failed tests, finding tests in a file and the run configurations.
 */
@TestInstance(TestInstance.Lifecycle.PER_CLASS)
class EngineIntegrationTest {
    private Path project;
    private Path buildRoot;
    private Engine engine;
    private final List<JsonObject> sent = new CopyOnWriteArrayList<>();

    @BeforeAll
    void sync() throws IOException {
        Path scratch = Path.of(System.getProperty("snout.test.scratch")).resolve("engine-" + System.nanoTime());
        project = scratch.resolve("run-app");
        SyncIntegrationTest.copy(Path.of(System.getProperty("snout.test.fixtures"), "run-app"), project);
        buildRoot = scratch.resolve("build");
        ProjectSync.Options options = new ProjectSync.Options();
        options.sync.projectDir = project.toFile();
        options.sync.gradleVersion = System.getProperty("snout.test.gradleVersion");
        options.sync.javaHome = Path.of(System.getProperty("java.home")).toFile();
        options.sync.sources = false;
        options.sync.buildRoot = buildRoot.toFile();
        options.sync.projectCacheDir = scratch.resolve("gradle").toFile();
        options.sync.gradleOutput = OutputStream.nullOutputStream();
        List<ProjectSync.Plan> plans = ProjectSync.plan(options);
        JsonObject model = ProjectSync.sync(options, plans);
        engine = new Engine(model, plans, sent::add, new PrintStream(OutputStream.nullOutputStream()), scratch.resolve("runs"));
    }

    @AfterAll
    void stop() {
        engine.shutdown();
    }

    @Test
    void listsTasksByGroupWithDescriptions() throws Exception {
        JsonObject result = engine.dispatch("snout/gradleTasks", new JsonObject(), false).getAsJsonObject();
        JsonObject build = result.getAsJsonArray("builds").get(0).getAsJsonObject();
        assertFalse(build.has("error"), build.toString());
        JsonObject app = null;
        for (JsonElement p : build.getAsJsonArray("projects")) {
            if (":app".equals(p.getAsJsonObject().get("project").getAsString())) {
                app = p.getAsJsonObject();
            }
        }
        assertNotNull(app, build.toString());
        assertEquals("run-app:app", app.get("module").getAsString());
        JsonObject demo = group(app, "demo");
        JsonObject hello = demo.getAsJsonArray("tasks").get(0).getAsJsonObject();
        assertEquals(":app:hello", hello.get("path").getAsString());
        assertEquals("Says hello.", hello.get("description").getAsString());
        assertTrue(group(app, "build").toString().contains(":app:classes"), app.toString());
        assertTrue(group(app, "verification").toString().contains(":app:test"), app.toString());
    }

    @Test
    void runsATaskWithOutputAndProgress() throws Exception {
        JsonObject params = new JsonObject();
        params.addProperty("project", ":app");
        params.addProperty("task", "hello");
        String runId = engine.dispatch("snout/runTask", params, false).getAsJsonObject().get("runId").getAsString();
        JsonObject done = await("snout/taskFinished", runId, p -> true);
        assertEquals("success", done.get("outcome").getAsString(), done.toString());
        assertTrue(lines(runId).contains("hello from the hello task"), lines(runId).toString());
        assertTrue(notifications("snout/taskProgress", runId).stream().anyMatch(p ->
            ":app:hello".equals(p.get("task").getAsString()) && "finished".equals(p.get("event").getAsString())
                && "success".equals(p.get("outcome").getAsString())), notifications("snout/taskProgress", runId).toString());
    }

    @Test
    void givesTheScalaCompilerRoomInEveryRun() throws Exception {
        JsonObject params = new JsonObject();
        params.addProperty("task", ":app:scalaHeap");
        String runId = engine.dispatch("snout/runTask", params, false).getAsJsonObject().get("runId").getAsString();
        JsonObject done = await("snout/taskFinished", runId, p -> true);
        assertEquals("success", done.get("outcome").getAsString(), done.toString());
        assertTrue(lines(runId).contains("scala heap " + ScalaBridge.SCALA_COMPILER_HEAP), lines(runId).toString());
    }

    @Test
    void reportsCompilerProblems() throws Exception {
        Path broken = project.resolve("app/src/main/java/demo/app/Broken.java");
        Files.writeString(broken, """
            package demo.app;

            final class Broken {
                int value() {
                    return missing();
                }
            }
            """);
        try {
            JsonObject params = new JsonObject();
            params.addProperty("task", ":app:compileJava");
            String runId = engine.dispatch("snout/runTask", params, false).getAsJsonObject().get("runId").getAsString();
            JsonObject done = await("snout/taskFinished", runId, p -> true);
            assertEquals("failed", done.get("outcome").getAsString(), done.toString());
            JsonArray problems = notifications("snout/buildProblems", runId).get(0).getAsJsonArray("problems");
            assertEquals(1, problems.size(), problems.toString());
            JsonObject problem = problems.get(0).getAsJsonObject();
            assertEquals(broken.toRealPath().toString(), Path.of(problem.get("file").getAsString()).toRealPath().toString());
            assertEquals(5, problem.get("line").getAsInt());
            assertEquals(16, problem.get("column").getAsInt());
            assertEquals("error", problem.get("severity").getAsString());
            assertTrue(problem.get("message").getAsString().startsWith("cannot find symbol"), problem.toString());
        } finally {
            Files.delete(broken);
        }
    }

    @Test
    void cancelsARun() throws Exception {
        Path slow = project.resolve("app/slow.gradle");
        JsonObject params = new JsonObject();
        params.addProperty("task", ":help");
        JsonArray args = new JsonArray();
        args.add("--init-script");
        Files.writeString(slow, "gradle.projectsEvaluated { gradle.rootProject.tasks.named('help').configure { it.doFirst { Thread.sleep(60000) } } }\n");
        args.add(slow.toString());
        params.add("args", args);
        String runId = engine.dispatch("snout/runTask", params, false).getAsJsonObject().get("runId").getAsString();
        await("snout/taskProgress", runId, p -> ":help".equals(p.get("task").getAsString()));
        JsonObject cancel = new JsonObject();
        cancel.addProperty("runId", runId);
        assertTrue(engine.dispatch("snout/cancelTask", cancel, false).getAsJsonObject().get("cancelled").getAsBoolean());
        JsonObject done = await("snout/taskFinished", runId, p -> true);
        assertEquals("cancelled", done.get("outcome").getAsString(), done.toString());
        assertTrue(done.get("millis").getAsLong() < 50_000, done.toString());
    }

    @Test
    void runsTestsWithEventsFiltersAndRerun() throws Exception {
        JsonObject params = new JsonObject();
        params.addProperty("module", "run-app:app");
        JsonObject filter = new JsonObject();
        filter.addProperty("kind", "class");
        filter.addProperty("class", "demo.app.MainTest");
        params.add("filter", filter);
        String runId = engine.dispatch("snout/runTests", params, false).getAsJsonObject().get("runId").getAsString();
        JsonObject done = await("snout/taskFinished", runId, p -> true);
        assertEquals("failed", done.get("outcome").getAsString(), done.toString());
        JsonObject tests = done.getAsJsonObject("tests");
        assertEquals(1, tests.get("passed").getAsInt(), done.toString());
        assertEquals(1, tests.get("failed").getAsInt(), done.toString());

        List<JsonObject> events = notifications("snout/testEvent", runId);
        JsonObject failure = events.stream().filter(e -> "finished".equals(e.get("event").getAsString())
            && "failsOnPurpose()".equals(e.get("name").getAsString())).findFirst().orElseThrow();
        assertEquals("failed", failure.get("outcome").getAsString());
        assertEquals("demo.app.MainTest", failure.get("className").getAsString());
        assertTrue(failure.get("message").getAsString().contains("two and two"), failure.toString());
        assertTrue(failure.get("stackTrace").getAsString().contains("MainTest.java"), failure.toString());
        assertTrue(failure.has("durationMillis"), failure.toString());
        assertTrue(events.stream().anyMatch(e -> "output".equals(e.get("event").getAsString())
            && e.get("text").getAsString().contains("adding in a test") && "adds()".equals(e.get("name").getAsString())), events.toString());
        assertTrue(events.stream().anyMatch(e -> "started".equals(e.get("event").getAsString()) && "suite".equals(e.get("kind").getAsString())
            && "demo.app.MainTest".equals(e.get("name").getAsString())), events.toString());
        assertTrue(events.stream().noneMatch(e -> String.valueOf(e.get("className")).contains("GreeterTest")), events.toString());
        assertTrue(lines(runId).stream().noneMatch(l -> l.contains(GradleRuns.TEST_EVENT_MARKER)));

        // The same tests again, though nothing changed: still run, still reported.
        JsonObject rerun = new JsonObject();
        rerun.addProperty("module", "run-app:app");
        rerun.addProperty("rerunFailed", true);
        String rerunId = engine.dispatch("snout/runTests", rerun, false).getAsJsonObject().get("runId").getAsString();
        JsonObject again = await("snout/taskFinished", rerunId, p -> true);
        JsonObject counts = again.getAsJsonObject("tests");
        assertEquals(0, counts.get("passed").getAsInt(), again.toString());
        assertEquals(1, counts.get("failed").getAsInt(), again.toString());

        JsonObject method = new JsonObject();
        method.addProperty("module", "run-app:app");
        JsonObject one = new JsonObject();
        one.addProperty("kind", "method");
        one.addProperty("class", "demo.app.GreeterTest");
        one.addProperty("method", "greets");
        method.add("filter", one);
        String methodId = engine.dispatch("snout/runTests", method, false).getAsJsonObject().get("runId").getAsString();
        JsonObject passed = await("snout/taskFinished", methodId, p -> true);
        assertEquals("success", passed.get("outcome").getAsString(), passed.toString());
        assertEquals(1, passed.getAsJsonObject("tests").get("passed").getAsInt(), passed.toString());
    }

    @Test
    void findsTestsInAFile() throws Exception {
        JsonObject params = new JsonObject();
        params.addProperty("path", project.resolve("app/src/test/java/demo/app/MainTest.java").toString());
        JsonObject result = engine.dispatch("snout/testsInFile", params, false).getAsJsonObject();
        assertEquals("run-app:app", result.get("module").getAsString());
        JsonObject type = result.getAsJsonArray("classes").get(0).getAsJsonObject();
        assertEquals("demo.app.MainTest", type.get("name").getAsString());
        assertEquals(2, type.getAsJsonArray("methods").size(), type.toString());
        assertEquals(8, type.getAsJsonArray("methods").get(0).getAsJsonObject().getAsJsonObject("range")
            .getAsJsonObject("start").get("line").getAsInt());
    }

    @Test
    void derivesRunConfigurations() throws Exception {
        JsonArray configs = engine.dispatch("snout/runConfigurations", new JsonObject(), false).getAsJsonObject()
            .getAsJsonArray("configurations");
        List<String> ids = new ArrayList<>();
        configs.forEach(c -> ids.add(c.getAsJsonObject().get("id").getAsString()));
        assertTrue(ids.contains("main:run-app:app:demo.app.Main"), ids.toString());
        assertTrue(ids.contains("gradle:run-app:app::app:run"), ids.toString());
        assertTrue(ids.contains("tests:run-app:app::app:test"), ids.toString());
        assertEquals(3, ids.size(), ids.toString());
    }

    // ---- helpers

    private static JsonObject group(JsonObject project, String name) {
        for (JsonElement g : project.getAsJsonArray("groups")) {
            if (name.equals(g.getAsJsonObject().get("name").getAsString())) {
                return g.getAsJsonObject();
            }
        }
        throw new AssertionError("no group " + name + " in " + project);
    }

    private List<JsonObject> notifications(String method, String runId) {
        List<JsonObject> out = new ArrayList<>();
        for (JsonObject m : sent) {
            if (method.equals(m.get("method").getAsString())) {
                JsonObject p = m.getAsJsonObject("params");
                if (runId.equals(p.get("runId").getAsString())) {
                    out.add(p);
                }
            }
        }
        return out;
    }

    private List<String> lines(String runId) {
        List<String> out = new ArrayList<>();
        notifications("snout/taskOutput", runId).forEach(p -> out.add(p.get("line").getAsString()));
        return out;
    }

    private JsonObject await(String method, String runId, Predicate<JsonObject> match) throws InterruptedException {
        long deadline = System.nanoTime() + Duration.ofMinutes(5).toNanos();
        while (System.nanoTime() < deadline) {
            for (JsonObject p : notifications(method, runId)) {
                if (match.test(p)) {
                    return p;
                }
            }
            Thread.sleep(50);
        }
        throw new AssertionError("no " + method + " for " + runId + "; output: " + lines(runId));
    }

    static JsonObject parse(String json) {
        return JsonParser.parseString(json).getAsJsonObject();
    }
}
