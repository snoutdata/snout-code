package com.snoutcode.engine.lsp;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;

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
 * project.
 */
class LspEndToEndTest {
    private static Path project;
    private static List<String> before;
    private static JsonObject report;

    @BeforeAll
    static void run() throws Exception {
        Path scratch = Path.of(System.getProperty("snout.test.scratch")).resolve("lsp-" + System.nanoTime());
        copy(Path.of(System.getProperty("snout.test.fixtures"), "java-app"), scratch.resolve("java-app"));
        project = scratch.resolve("java-app/project");
        before = listing(scratch.resolve("java-app"));

        String app = "app/src/main/java/demo/app/App.java";
        JsonObject script = JsonParser.parseString("""
            {
              "engineArgs": ["--cache-dir", "%s", "--gradle-version", "%s", "--java-home", "%s", "--no-sources"],
              "env": { "JAVA_HOME": "%s" },
              "open": ["%s"],
              "checks": [
                { "name": "definition in another project", "kind": "definition", "file": "%s",
                  "find": "Lib.hello", "offset": 4, "expect": "lib/src/main/java/demo/lib/Lib.java" },
                { "name": "definition in the shared folder", "kind": "definition", "file": "%s",
                  "find": "shout()", "expect": "shared/src/main/java/demo/shared/Greeting.java" },
                { "name": "completion", "kind": "completion", "file": "%s",
                  "find": "Lib.hello", "offset": 4, "expect": "hello" },
                { "name": "references", "kind": "references", "file": "lib/src/main/java/demo/lib/Lib.java",
                  "find": "hello(", "min": 3 }
              ]
            }""".formatted(
            json(scratch.resolve("cache").toString()), System.getProperty("snout.test.gradleVersion"),
            json(System.getProperty("java.home")), json(System.getProperty("java.home")), app, app, app, app)).getAsJsonObject();
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
        assertEquals(4, checks.size());
        for (JsonElement c : checks) {
            assertTrue(c.getAsJsonObject().get("pass").getAsBoolean(), c.toString());
        }
    }

    @Test
    void leavesTheProjectUntouched() throws IOException {
        assertEquals(before, listing(project.getParent()));
    }

    @Test
    void movesBuildDirectoriesIntoTheCache() throws IOException {
        Path cache = project.getParent().getParent().resolve("cache");
        Path model;
        try (Stream<Path> files = Files.walk(cache.resolve("jdtls"))) {
            model = files.filter(p -> p.getFileName().toString().equals("model.json")).findFirst().orElseThrow();
        }
        JsonObject build = JsonParser.parseString(Files.readString(model)).getAsJsonObject().getAsJsonArray("builds").get(0).getAsJsonObject();
        for (JsonElement p : build.getAsJsonArray("projects")) {
            String buildDir = p.getAsJsonObject().get("buildDir").getAsString();
            assertTrue(Path.of(buildDir).startsWith(cache.resolve("build")), buildDir);
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
