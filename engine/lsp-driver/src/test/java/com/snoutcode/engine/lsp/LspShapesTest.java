package com.snoutcode.engine.lsp;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;
import org.junit.jupiter.api.AfterAll;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.TestInstance;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Duration;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.stream.Stream;

import static org.junit.jupiter.api.Assertions.assertEquals;

/**
 * Correct code in shapes that real builds have and that once showed errors (the fixture
 * maven-shapes): Java modules, whose dependencies must be modules too; Lombok, which jdtls runs
 * as an agent; a Java level that only a parent's pluginManagement sets; a module-info.java the
 * build leaves out of sources of an older level; and a class a source generator makes, which the
 * engine runs after the sync. Every file must open without an error.
 */
@TestInstance(TestInstance.Lifecycle.PER_CLASS)
class LspShapesTest {
    private static final Duration LONG = Duration.ofMinutes(10);

    private Path project;
    private LspClient client;

    @BeforeAll
    void start() throws Exception {
        Path scratch = Path.of(System.getProperty("snout.test.scratch")).resolve("shapes-" + System.nanoTime());
        project = scratch.resolve("maven-shapes");
        LspEndToEndTest.copy(Path.of(System.getProperty("snout.test.fixtures"), "maven-shapes"), project);
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
        driver.awaitReady(LONG);
        client.request("java/buildWorkspace", new JsonPrimitive(false), LONG);
    }

    @AfterAll
    void stop() throws IOException {
        if (client != null) {
            client.close();
        }
    }

    @Test
    void everyFileOpensWithoutErrors() throws Exception {
        // The legacy module-info.java is out of the build, as javac has it: no error from the build.
        // (Opened, jdtls would parse it on its own at Java 8, and find module syntax wrong.)
        Path apart = project.resolve("legacy/src/module-info.java");
        List<Path> files;
        try (Stream<Path> walk = Files.walk(project)) {
            // The template is the generator's input, not a source.
            files = walk.filter(f -> f.toString().endsWith(".java") && !f.equals(apart) && !f.toString().contains("java-templates"))
                .sorted().toList();
        }
        for (Path f : files) {
            client.didOpen(f);
        }
        client.awaitDiagnosticsFor(files, Duration.ofMinutes(2));
        client.awaitDiagnosticsQuiet(Duration.ofSeconds(3), Duration.ofMinutes(2));
        List<String> errors = new ArrayList<>();
        for (Path f : files) {
            JsonArray d = client.diagnostics.get(LspClient.uri(f));
            if (d == null) {
                continue;
            }
            d.forEach(x -> {
                JsonObject o = x.getAsJsonObject();
                if (o.get("severity").getAsInt() == 1) {
                    errors.add(project.relativize(f) + ": " + o.get("message").getAsString());
                }
            });
        }
        assertEquals(List.of(), errors);
        JsonArray left = client.diagnostics.get(LspClient.uri(apart));
        assertEquals(0, left == null ? 0 : left.size(), String.valueOf(left));
    }
}
