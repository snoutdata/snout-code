package com.snoutcode.engine.cli;

import org.junit.jupiter.api.Test;

import java.io.File;
import java.io.IOException;
import java.io.OutputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.stream.Stream;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/** The engine's own Gradle daemons: in their registry, with the short idle time, and stopped. */
class DaemonsTest {
    @Test
    void aSyncRunsOnAPrivateDaemonThatStopStops() throws IOException {
        Path scratch = Path.of(System.getProperty("snout.test.scratch"));
        Path root = scratch.resolve("daemons-" + System.nanoTime());
        Files.createDirectories(root);
        Files.writeString(root.resolve("settings.gradle"), "rootProject.name = 'daemons'\n");
        Files.writeString(root.resolve("build.gradle"), "apply plugin: 'java'\n");
        File registry = Daemons.registry(root.resolve("cache"), root.toFile());

        Sync.Options options = new Sync.Options();
        options.projectDir = root.toFile();
        options.gradleVersion = System.getProperty("snout.test.gradleVersion");
        options.sources = false;
        options.gradleOutput = OutputStream.nullOutputStream();
        options.daemonRegistry = registry;
        Sync.run(options);

        List<ProcessHandle> daemons = Daemons.running(registry.toPath());
        assertEquals(1, daemons.size(), "one daemon in " + registry);
        String log;
        try (Stream<Path> files = Files.walk(registry.toPath())) {
            Path file = files.filter(f -> f.getFileName().toString().endsWith(".out.log")).findFirst().orElseThrow();
            log = Files.readString(file);
        }
        assertTrue(log.contains("idleTimeout=" + Daemons.IDLE_MINUTES * 60_000 + ","), "the daemon's idle time");

        assertEquals(1, Daemons.stop(registry, 10_000, null));
        assertFalse(daemons.get(0).isAlive());
        assertTrue(Daemons.running(registry.toPath()).isEmpty());
        assertEquals(0, Daemons.stop(registry, 10_000, null));
    }

    @Test
    void noRegistryStopsNothing() {
        assertEquals(0, Daemons.stop(new File(System.getProperty("snout.test.scratch"), "no-such-registry"), 100, null));
    }
}
