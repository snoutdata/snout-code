package com.snoutcode.engine.lsp;

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
import java.util.stream.Stream;

import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * Files that appear while the engine is not running (a {@code git pull} with the app closed) are
 * compiled at the next start: in the build's own folder and in a folder outside it that the build
 * compiles into itself ({@code ../core/src/main/java}), which jdtls holds as a linked folder and
 * would otherwise go on seeing as it was in the last session.
 */
@TestInstance(TestInstance.Lifecycle.PER_CLASS)
class LspFilesBetweenSessionsTest {
    private static final Duration LONG = Duration.ofMinutes(5);

    private Path scratch;
    private Path app;
    private Path core;
    private LspClient client;

    @BeforeAll
    void start() throws Exception {
        scratch = Path.of(System.getProperty("snout.test.scratch")).resolve("between-" + System.nanoTime());
        app = scratch.resolve("app");
        core = scratch.resolve("core/src/main/java/demo/core");
        Files.createDirectories(core);
        Files.createDirectories(app.resolve("src/main/java/demo"));
        Files.writeString(app.resolve("settings.gradle"), "rootProject.name = 'app'\n");
        Files.writeString(app.resolve("build.gradle"), """
            plugins { id 'java' }
            sourceSets.main.java.srcDirs += ['../core/src/main/java']
            """);
        Files.writeString(app.resolve("src/main/java/demo/App.java"), """
            package demo;

            public final class App {
                public static void main(String[] args) {
                    System.out.println(demo.core.Base.name());
                }
            }
            """);
        Files.writeString(core.resolve("Base.java"), """
            package demo.core;

            public final class Base {
                public static String name() { return "base"; }
            }
            """);

        session().close();
        Files.writeString(core.resolve("Pulled.java"), "package demo.core;\n\npublic final class Pulled {}\n");
        Files.writeString(app.resolve("src/main/java/demo/PulledToo.java"), "package demo;\n\npublic final class PulledToo {}\n");
        client = session();
    }

    private LspClient session() throws Exception {
        List<String> command = List.of(System.getProperty("snout.test.engine"), "lsp", app.toString(),
            "--cache-dir", scratch.resolve("cache").toString(), "--gradle-version", System.getProperty("snout.test.gradleVersion"),
            "--java-home", System.getProperty("java.home"), "--no-sources");
        LspClient c = LspClient.start(command, Map.of("JAVA_HOME", System.getProperty("java.home")),
            scratch.resolve("stderr-" + System.nanoTime() + ".log").toFile());
        Driver driver = new Driver(app, c);
        driver.initialize(new JsonObject());
        driver.awaitReady(Duration.ofMinutes(10));
        c.request("java/buildWorkspace", new JsonPrimitive(false), LONG);
        return c;
    }

    @AfterAll
    void stop() throws IOException {
        if (client != null) {
            client.close();
        }
    }

    /** The class file jdtls wrote for {@code name}, if any. */
    private boolean compiled(String name) throws IOException {
        String file = name.replace('.', '/') + ".class";
        try (Stream<Path> found = Files.walk(scratch.resolve("cache"))) {
            return found.anyMatch(p -> p.toString().replace('\\', '/').endsWith("/bin/main/" + file));
        }
    }

    @Test
    void compilesWhatAppearedInTheBuildsFolderMeanwhile() throws IOException {
        assertTrue(compiled("demo.App"), "the first session compiled the app");
        assertTrue(compiled("demo.PulledToo"), "a file new in the build's folder is compiled");
    }

    @Test
    void compilesWhatAppearedInAFolderOutsideTheBuildMeanwhile() throws IOException {
        assertTrue(compiled("demo.core.Base"), "the first session compiled the outside folder");
        assertTrue(compiled("demo.core.Pulled"), "a file new in the outside folder is compiled");
    }
}
