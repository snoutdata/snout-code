package com.snoutcode.engine.lsp;

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
import java.time.Duration;
import java.util.List;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * A launch of Java code that uses Scala compiles the Scala first, when it is not up to date,
 * and is refused, with the compiler's reason, when the Scala does not compile: jdtls would
 * have compiled the Java without the Scala classes, and the app would fail to load them.
 */
@TestInstance(TestInstance.Lifecycle.PER_CLASS)
class LspScalaLaunchTest {
    private static final Duration LONG = Duration.ofMinutes(5);

    private Path project;
    private LspClient client;

    @BeforeAll
    void start() throws Exception {
        Path scratch = Path.of(System.getProperty("snout.test.scratch")).resolve("scala-launch-" + System.nanoTime());
        LspEndToEndTest.copy(Path.of(System.getProperty("snout.test.engineFixtures"), "scala-app"), scratch.resolve("scala-app"));
        project = scratch.resolve("scala-app");
        Files.writeString(project.resolve("src/main/scala/demo/Main.java"), """
            package demo;

            public final class Main {
                public static void main(String[] args) {
                    System.out.println("calc " + UseCalc.run());
                }
            }
            """);
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
    void refusesALaunchWhoseScalaDoesNotCompile() throws Exception {
        JsonObject config = null;
        for (JsonElement c : client.request("snout/runConfigurations", new JsonObject(), LONG).getAsJsonObject()
            .getAsJsonArray("configurations")) {
            if ("demo.Main".equals(c.getAsJsonObject().get("mainClass") == null ? null : c.getAsJsonObject().get("mainClass").getAsString())) {
                config = c.getAsJsonObject();
            }
        }
        assertNotNull(config, "demo.Main is offered");
        JsonObject params = new JsonObject();
        params.add("config", config);

        Path calc = project.resolve("src/main/scala/demo/Calc.scala");
        String good = Files.readString(calc);
        Files.writeString(calc, good.replace("x * 2", "x *"));
        IOException refused = assertThrows(IOException.class, () -> client.request("snout/run", params, LONG));
        assertTrue(refused.getMessage().contains("did not compile"), refused.getMessage());

        Files.writeString(calc, good);
        String runId = client.request("snout/run", params, LONG).getAsJsonObject().get("runId").getAsString();
        JsonObject done = client.awaitNotification("snout/taskFinished", runId, p -> true, LONG);
        assertEquals(0, done.get("exitCode").getAsInt(), done + " " + client.outputLines(runId));
        assertTrue(client.outputLines(runId).stream().anyMatch(l -> l.startsWith("calc ")), client.outputLines(runId).toString());
    }
}
