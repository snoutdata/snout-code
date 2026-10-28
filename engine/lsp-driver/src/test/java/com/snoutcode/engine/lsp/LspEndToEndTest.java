package com.snoutcode.engine.lsp;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.TestInstance;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.List;
import java.util.stream.Stream;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * Runs {@code snout-engine lsp} on a small two-project build with a source folder outside it,
 * and checks through LSP that jdtls imported it from our model: no errors, navigation across
 * projects and into the shared folder, completion, references, and nothing written into the
 * project. The fixture's Gradle runs on the JVM running the tests (Java 21, jdtls's own);
 * {@link LspEndToEndJdk17Test} runs it on Java 17.
 */
@TestInstance(TestInstance.Lifecycle.PER_CLASS)
class LspEndToEndTest {
    private Path project;
    private List<String> before;
    private JsonObject report;

    /** The JDK the fixture's Gradle runs on, and so the JDK of its JDT projects. */
    String gradleJavaHome() {
        return System.getProperty("java.home");
    }

    @BeforeAll
    void run() throws Exception {
        Path scratch = Path.of(System.getProperty("snout.test.scratch")).resolve("lsp-" + System.nanoTime());
        copy(Path.of(System.getProperty("snout.test.fixtures"), "java-app"), scratch.resolve("java-app"));
        project = scratch.resolve("java-app/project");
        before = listing(scratch.resolve("java-app"));

        String app = "app/src/main/java/demo/app/App.java";
        String lib = "lib/src/main/java/demo/lib/Lib.java";
        // A method's references need its binding, which resolves against the JRE container
        // under the project's compliance: these failed when the project's JDK was not jdtls's.
        JsonObject script = JsonParser.parseString("""
            {
              "engineArgs": ["--cache-dir", "%1$s", "--gradle-version", "%2$s", "--java-home", "%3$s", "--no-sources"],
              "env": { "JAVA_HOME": "%4$s" },
              "open": ["%5$s"],
              "checks": [
                { "name": "definition in another project", "kind": "definition", "file": "%5$s",
                  "find": "Lib.hello", "offset": 4, "expect": "lib/src/main/java/demo/lib/Lib.java" },
                { "name": "definition in the shared folder", "kind": "definition", "file": "%5$s",
                  "find": "shout()", "expect": "shared/src/main/java/demo/shared/Greeting.java" },
                { "name": "completion", "kind": "completion", "file": "%5$s",
                  "find": "Lib.hello", "offset": 4, "expect": "hello" },
                { "name": "references", "kind": "references", "file": "%6$s",
                  "find": "hello(", "min": 3 },
                { "name": "references to a method from a call", "kind": "references", "file": "%5$s",
                  "find": "Lib.hello", "offset": 4, "min": 3 },
                { "name": "references to a JDK method", "kind": "references", "file": "%5$s",
                  "find": "println", "min": 1 },
                { "name": "references to a type", "kind": "references", "file": "%5$s",
                  "find": "Greeting greet", "min": 3 }
              ]
            }""".formatted(
            json(scratch.resolve("cache").toString()), System.getProperty("snout.test.gradleVersion"),
            json(gradleJavaHome()), json(System.getProperty("java.home")), app, lib)).getAsJsonObject();
        report = Driver.run(Path.of(System.getProperty("snout.test.engine")), project, script, scratch.resolve("stderr.log"));
        Files.writeString(scratch.resolve("report.json"), report.toString());
    }

    @Test
    void importsWithoutErrors() {
        JsonObject diagnostics = report.getAsJsonObject("diagnostics");
        assertEquals(0, diagnostics.get("errors").getAsInt(), report.toString());
        assertEquals(0, diagnostics.getAsJsonArray("nonJavaErrors").size(), report.toString());
        assertTrue(report.getAsJsonObject("firstDiagnosticsMillis").entrySet().stream().allMatch(e -> !e.getValue().isJsonNull()),
            report.toString());
        List<String> statuses = new ArrayList<>();
        report.getAsJsonArray("statuses").forEach(e -> statuses.add(e.getAsString()));
        assertTrue(statuses.contains("ProjectStatus: OK"), statuses.toString());
    }

    @Test
    void navigationCompletionAndReferencesWork() {
        JsonArray checks = report.getAsJsonArray("checks");
        assertEquals(7, checks.size());
        for (JsonElement c : checks) {
            assertTrue(c.getAsJsonObject().get("pass").getAsBoolean(), c.toString());
        }
    }

    @Test
    void leavesTheProjectUntouched() throws IOException {
        assertEquals(before, listing(project.getParent()));
    }

    @Test
    void syncsOnTheGivenJdk() throws IOException {
        assertEquals(Path.of(gradleJavaHome()).toRealPath(), Path.of(model().get("gradleJavaHome").getAsString()).toRealPath());
    }

    @Test
    void movesBuildDirectoriesIntoTheCache() throws IOException {
        Path cache = project.getParent().getParent().resolve("cache");
        JsonObject build = model().getAsJsonArray("builds").get(0).getAsJsonObject();
        for (JsonElement p : build.getAsJsonArray("projects")) {
            String buildDir = p.getAsJsonObject().get("buildDir").getAsString();
            assertTrue(Path.of(buildDir).startsWith(cache.resolve("build")), buildDir);
        }
    }

    private JsonObject model() throws IOException {
        Path cache = project.getParent().getParent().resolve("cache");
        try (Stream<Path> files = Files.walk(cache.resolve("jdtls"))) {
            Path model = files.filter(p -> p.getFileName().toString().equals("model.json")).findFirst().orElseThrow();
            return JsonParser.parseString(Files.readString(model)).getAsJsonObject();
        }
    }

    private static List<String> listing(Path dir) throws IOException {
        try (Stream<Path> files = Files.walk(dir)) {
            return files.map(p -> dir.relativize(p).toString()).sorted().toList();
        }
    }

    private static String json(String s) {
        return s.replace("\\", "\\\\").replace("\"", "\\\"");
    }

    private static void copy(Path from, Path to) throws IOException {
        try (Stream<Path> files = Files.walk(from)) {
            for (Path source : files.toList()) {
                Path target = to.resolve(from.relativize(source).toString());
                if (Files.isDirectory(source)) {
                    Files.createDirectories(target);
                } else {
                    Files.copy(source, target, StandardCopyOption.REPLACE_EXISTING);
                }
            }
        }
    }
}
