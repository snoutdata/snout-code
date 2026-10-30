package com.snoutcode.engine.cli;

import com.google.gson.JsonObject;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;

import java.io.File;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * Runs Maven goals as the engine does for the app: Maven downloaded into the cache (no Maven
 * installed), a module's tests alone with m2e's workspace reader resolving the library it
 * uses, test events from the listener, a failed test rerun by itself, and compiler problems.
 */
class MavenRunsTest {
    private static Path root;
    private static List<Modules.Ref> reactor;
    private static ProjectSync.Plan plan;

    @BeforeAll
    static void sync() throws IOException {
        root = Path.of(System.getProperty("snout.test.scratch")).resolve("maven-runs-" + System.nanoTime()).resolve("maven-multi");
        SyncIntegrationTest.copy(Path.of(System.getProperty("snout.test.fixtures"), "maven-multi"), root);
        ProjectSync.Options options = MavenSyncTest.options(root, List.of());
        plan = ProjectSync.plan(options).get(0);
        JsonObject model = ProjectSync.sync(options, List.of(plan));
        reactor = Modules.all(model);
        MavenHome.cacheDir(Path.of(System.getProperty("snout.test.cache")));
        File[] m2e = new File(System.getProperty("snout.test.jdtlsPlugins"))
            .listFiles((d, n) -> n.startsWith("org.eclipse.m2e.workspace.cli_"));
        System.setProperty("snout.m2eWorkspace", m2e[0].getAbsolutePath());
        System.setProperty("snout.junitListener", System.getProperty("snout.test.junitListener"));
    }

    /** Collects what a run sends. */
    static final class Collect implements GradleRuns.Client {
        final List<String> lines = Collections.synchronizedList(new ArrayList<>());
        final List<JsonObject> events = Collections.synchronizedList(new ArrayList<>());
        final List<JsonObject> progress = Collections.synchronizedList(new ArrayList<>());
        JsonObject problems;

        @Override
        public void notify(String method, JsonObject params) {
            switch (method) {
                case "snout/taskOutput" -> lines.add(params.get("line").getAsString());
                case "snout/testEvent" -> events.add(params);
                case "snout/taskProgress" -> progress.add(params);
                case "snout/buildProblems" -> problems = params;
                default -> {
                }
            }
        }

        List<String> finished(String outcome) {
            List<String> out = new ArrayList<>();
            for (JsonObject e : events) {
                if ("finished".equals(e.get("event").getAsString()) && "test".equals(e.get("kind").getAsString())
                    && outcome.equals(e.get("outcome").getAsString())) {
                    out.add(e.get("name").getAsString());
                }
            }
            return out;
        }
    }

    private static MavenRuns.Invocation tests(String filter) {
        MavenRuns.Invocation inv = new MavenRuns.Invocation();
        inv.plan = plan;
        inv.reactor = reactor;
        inv.module = reactor.stream().filter(m -> ":app".equals(m.gradlePath())).findFirst().orElseThrow();
        inv.goals.add("test");
        inv.tests = true;
        inv.testFilter = filter;
        inv.runDir = root.resolveSibling("runs");
        return inv;
    }

    @Test
    void runsAModulesTestsAloneAndRerunsTheFailedOne() throws IOException {
        Collect c = new Collect();
        GradleRuns.Run run = new GradleRuns.Run("run-1");
        JsonObject done = MavenRuns.execute(tests(null), run, c);
        String output = String.join("\n", c.lines);
        assertEquals("failed", done.get("outcome").getAsString(), output);
        assertTrue(c.finished("passed").contains("adds()"), c.events.toString());
        assertTrue(c.finished("passed").contains("usesTheLibrarysTests()"), "the library's test jar: " + c.events);
        assertEquals(List.of("passesOnceFixed()"), c.finished("failed"), output);
        assertTrue(output.contains("> mvn -B -ntp -f"), "the downloaded Maven runs: " + output);
        // The library compiled first, but its own tests did not run.
        assertTrue(c.events.stream().noneMatch(e -> e.toString().contains("GreeterTest")), c.events.toString());
        assertTrue(c.progress.stream().anyMatch(p -> p.get("task").getAsString().startsWith("app:surefire:test")), c.progress.toString());
        assertEquals(List.of("demo.app.MainTest", "passesOnceFixed"), List.of(run.failedTests.get(0)));

        Files.writeString(root.resolve("app/fixed.txt"), "fixed");
        Collect again = new Collect();
        JsonObject rerun = MavenRuns.execute(tests(MavenRuns.rerunFilter(run.failedTests)), new GradleRuns.Run("run-2"), again);
        assertEquals("success", rerun.get("outcome").getAsString(), String.join("\n", again.lines));
        assertEquals(List.of("passesOnceFixed()"), again.finished("passed"));
    }

    @Test
    void reportsCompilerProblems() throws IOException {
        Path broken = root.resolveSibling("broken");
        SyncIntegrationTest.copy(root, broken);
        Files.writeString(broken.resolve("core/src/main/java/demo/core/Broken.java"),
            "package demo.core;\n\nclass Broken {\n    int x = missing;\n}\n");
        MavenRuns.Invocation inv = new MavenRuns.Invocation();
        inv.plan = ProjectSync.plan(MavenSyncTest.options(broken, List.of())).get(0);
        inv.goals.add("compile");
        Collect c = new Collect();
        JsonObject done = MavenRuns.execute(inv, new GradleRuns.Run("run-3"), c);
        assertEquals("failed", done.get("outcome").getAsString());
        String problems = c.problems.toString();
        assertTrue(problems.contains("Broken.java") && problems.contains("\"line\":4") && problems.contains("cannot find symbol"), problems + String.join("\n", c.lines));
    }
}
