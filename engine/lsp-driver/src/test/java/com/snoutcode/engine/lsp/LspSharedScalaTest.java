package com.snoutcode.engine.lsp;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.TestInstance;

import java.nio.file.Files;
import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * Two Gradle builds, {@code alpha} and {@code beta}, compile the folder {@code shared} (Java and
 * Scala that use each other) into themselves through their Scala source sets. The engine
 * compiles the shared Scala once, each build's own Scala against it, and jdtls sees every type
 * once with no errors. A second start finds the builds and the Scala unchanged: it runs neither
 * the sync nor a compile, and jdtls is ready at once.
 */
@TestInstance(TestInstance.Lifecycle.PER_CLASS)
class LspSharedScalaTest {
    private Path scratch;
    private JsonObject cold;
    private JsonObject warm;
    private String coldLog;
    private String warmLog;

    @BeforeAll
    void run() throws Exception {
        scratch = Path.of(System.getProperty("snout.test.scratch")).resolve("shared-scala-" + System.nanoTime());
        Path folder = scratch.resolve("shared-scala");
        LspEndToEndTest.copy(Path.of(System.getProperty("snout.test.fixtures"), "shared-scala"), folder);
        String alpha = "alpha/src/main/scala/demo/alpha/Alpha.java";
        String beta = "beta/src/main/scala/demo/beta/Beta.java";
        String words = "shared/src/main/scala/demo/shared/Words.java";
        String java = System.getProperty("java.home");
        JsonObject script = JsonParser.parseString("""
            {
              "engineArgs": ["--cache-dir", "%1$s", "--gradle-version", "%2$s", "--java-home", "%3$s", "--no-sources", "--no-debugger"],
              "env": { "JAVA_HOME": "%3$s" },
              "open": ["%4$s", "%5$s", "%6$s"],
              "checks": [
                { "name": "alpha's Java to the shared Scala", "kind": "definition", "file": "%4$s",
                  "find": "Tone.loud", "offset": 5, "expect": "demo.shared/Tone.scala", "single": true },
                { "name": "beta's Java to its own Scala", "kind": "definition", "file": "%5$s",
                  "find": "BetaTune.tune", "offset": 9, "expect": "demo.beta/BetaTune.scala", "single": true },
                { "name": "the shared Java to the shared Scala", "kind": "definition", "file": "%6$s",
                  "find": "Tone.loud", "offset": 5, "expect": "demo.shared/Tone.scala", "single": true },
                { "name": "references list each use once", "kind": "references", "file": "%6$s",
                  "find": "twice(", "min": 3, "max": 3, "distinct": true }
              ]
            }""".formatted(
            LspEndToEndTest.json(scratch.resolve("cache").toString()), System.getProperty("snout.test.gradleVersion"),
            LspEndToEndTest.json(java), alpha, beta, words)).getAsJsonObject();
        Path engine = Path.of(System.getProperty("snout.test.engine"));
        cold = Driver.run(engine, folder, script, scratch.resolve("cold.log"));
        warm = Driver.run(engine, folder, script, scratch.resolve("warm.log"));
        Files.writeString(scratch.resolve("cold.json"), cold.toString());
        Files.writeString(scratch.resolve("warm.json"), warm.toString());
        coldLog = Files.readString(scratch.resolve("cold.log"));
        warmLog = Files.readString(scratch.resolve("warm.log"));
    }

    @Test
    void compilesTheSharedScalaOnceAndJdtlsSeesItOnce() {
        assertTrue(coldLog.contains("compiled the shared Scala of shared once for 2 compile(s)"), coldLog);
        assertEquals(0, cold.getAsJsonObject("diagnostics").get("errors").getAsInt(), cold.toString());
        for (JsonElement c : cold.getAsJsonArray("checks")) {
            assertTrue(c.getAsJsonObject().get("pass").getAsBoolean(), c.toString());
        }
        assertTrue(cold.get("projectReadyMillis").getAsLong() > 0, cold.toString());
        // The shared Scala's classes come from the one compile, with their Scala source attached.
        JsonObject toShared = cold.getAsJsonArray("checks").get(0).getAsJsonObject();
        assertTrue(toShared.toString().contains("bridge") && toShared.toString().contains("classes-scala"), toShared.toString());
        assertEquals("attached source", toShared.get("classFileSource").getAsString(), toShared.toString());
    }

    @Test
    void aSecondStartSkipsTheSyncAndTheCompile() {
        assertTrue(warmLog.contains("the builds did not change since the last sync"), warmLog);
        assertTrue(warmLog.contains("the shared Scala of shared is up to date"), warmLog);
        assertTrue(warmLog.contains("Scala in alpha is up to date") && warmLog.contains("Scala in beta is up to date"), warmLog);
        assertFalse(warmLog.contains("snout-engine: syncing"), warmLog);
        assertEquals(0, warm.getAsJsonObject("diagnostics").get("errors").getAsInt(), warm.toString());
        for (JsonElement c : warm.getAsJsonArray("checks")) {
            assertTrue(c.getAsJsonObject().get("pass").getAsBoolean(), c.toString());
        }
    }
}
