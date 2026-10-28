package com.snoutcode.engine.lsp;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;
import org.junit.jupiter.api.AfterAll;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.TestInstance;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.attribute.FileTime;
import java.time.Duration;
import java.util.List;
import java.util.Map;
import java.util.stream.Stream;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * The Scala bridge after a Scala file changes: {@code snout/scalaChanged} recompiles through
 * Gradle's incremental Scala compile (only the changed file's classes are rewritten), and jdtls
 * sees the new Scala method without a restart, so the Java code that calls it compiles.
 */
@TestInstance(TestInstance.Lifecycle.PER_CLASS)
class LspScalaIncrementalTest {
    private static final Duration LONG = Duration.ofMinutes(5);

    private Path project;
    private Path cache;
    private LspClient client;

    @BeforeAll
    void start() throws Exception {
        Path scratch = Path.of(System.getProperty("snout.test.scratch")).resolve("scala-" + System.nanoTime());
        LspEndToEndTest.copy(Path.of(System.getProperty("snout.test.engineFixtures"), "scala-app"), scratch.resolve("scala-app"));
        project = scratch.resolve("scala-app");
        cache = scratch.resolve("cache");
        List<String> command = List.of(System.getProperty("snout.test.engine"), "lsp", project.toString(),
            "--cache-dir", cache.toString(), "--gradle-version", System.getProperty("snout.test.gradleVersion"),
            "--java-home", System.getProperty("java.home"), "--no-sources", "--no-debugger");
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
    void recompilesChangedScalaAndJdtlsSeesIt() throws Exception {
        Path java = project.resolve("src/main/scala/demo/UseCalc.java");
        Path calc = project.resolve("src/main/scala/demo/Calc.scala");
        client.didOpen(java);
        client.awaitDiagnosticsFor(List.of(java), Duration.ofMinutes(2));
        client.awaitDiagnosticsQuiet(Duration.ofSeconds(2), LONG);
        assertEquals(0, errors(java), client.diagnostics.toString());

        // Java calls a Scala method that does not exist yet.
        String edited = Files.readString(java).replace("Calc.twice(2)", "Calc.thrice(2)");
        Files.writeString(java, edited);
        change(java, edited, 2);
        awaitErrors(java, true);

        Path other = classFile("Other.class");
        FileTime otherBefore = Files.getLastModifiedTime(other);
        FileTime calcBefore = Files.getLastModifiedTime(classFile("Calc$.class"));
        Thread.sleep(1100); // file times have a one second resolution on some file systems

        Files.writeString(calc, Files.readString(calc).replace("def twice", "def thrice(x: Int): Int = x * 3\n  def twice"));
        JsonObject params = new JsonObject();
        JsonArray files = new JsonArray();
        files.add(LspClient.uri(calc));
        params.add("files", files);
        JsonObject result = client.request("snout/scalaChanged", params, LONG).getAsJsonObject();
        JsonObject build = result.getAsJsonArray("builds").get(0).getAsJsonObject();
        assertTrue(build.get("error").isJsonNull(), result.toString());
        assertEquals(":compileScala", build.getAsJsonArray("tasks").get(0).getAsString(), result.toString());

        // Incremental: Calc was recompiled, Other was not.
        assertTrue(Files.getLastModifiedTime(classFile("Calc$.class")).compareTo(calcBefore) > 0);
        assertEquals(otherBefore, Files.getLastModifiedTime(other));

        // jdtls reads the refreshed class folder: the call resolves now.
        change(java, edited, 3);
        awaitErrors(java, false);
    }

    private void change(Path file, String text, int version) throws IOException {
        JsonObject params = new JsonObject();
        JsonObject document = new JsonObject();
        document.addProperty("uri", LspClient.uri(file));
        document.addProperty("version", version);
        params.add("textDocument", document);
        JsonArray changes = new JsonArray();
        JsonObject whole = new JsonObject();
        whole.addProperty("text", text);
        changes.add(whole);
        params.add("contentChanges", changes);
        client.notify("textDocument/didChange", params);
        JsonObject saved = new JsonObject();
        JsonObject savedDocument = new JsonObject();
        savedDocument.addProperty("uri", LspClient.uri(file));
        saved.add("textDocument", savedDocument);
        client.notify("textDocument/didSave", saved);
    }

    private void awaitErrors(Path file, boolean wanted) throws InterruptedException {
        long deadline = System.nanoTime() + LONG.toNanos();
        while (System.nanoTime() < deadline && (errors(file) > 0) != wanted) {
            Thread.sleep(100);
        }
        assertEquals(wanted, errors(file) > 0, client.diagnostics.toString());
    }

    private int errors(Path file) {
        JsonArray list = client.diagnostics.get(LspClient.uri(file));
        int n = 0;
        if (list != null) {
            for (JsonElement d : list) {
                JsonObject diagnostic = d.getAsJsonObject();
                if (!diagnostic.has("severity") || diagnostic.get("severity").getAsInt() == 1) {
                    n++;
                }
            }
        }
        return n;
    }

    /** A class Gradle compiled, in the build directory under the cache. */
    private Path classFile(String name) throws IOException {
        try (Stream<Path> walk = Files.walk(cache.resolve("build"))) {
            return walk.filter(p -> p.getFileName().toString().equals(name) && p.toString().contains("classes/scala/main/"))
                .findFirst().orElseThrow(() -> new IOException("no " + name + " under " + cache));
        }
    }
}
