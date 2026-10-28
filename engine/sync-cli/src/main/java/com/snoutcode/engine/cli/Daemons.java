package com.snoutcode.engine.cli;

import org.gradle.tooling.GradleConnector;
import org.gradle.tooling.internal.consumer.DefaultGradleConnector;

import java.io.File;
import java.io.IOException;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.DirectoryStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Instant;
import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.TimeUnit;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * The Gradle daemons of the engine: private to it, and stopped when it exits.
 *
 * <p>Each project folder gets a daemon registry of its own under the cache directory
 * ({@code daemons/<hash>}). A Gradle client finds daemons only in its registry, so the engine
 * never runs a build on a daemon the user started from a terminal, and the user's builds never
 * land on one of the engine's. Nothing else about the daemon changes: its JVM, JVM arguments
 * ({@code org.gradle.jvmargs}) and Gradle user home are the build's.
 *
 * <p>The engine's daemons stop after {@link #IDLE_MINUTES} minutes without a build, instead of
 * Gradle's three hours, in case the engine dies before it can stop them; and the engine stops
 * them when it exits ({@link #stop}).
 *
 * <p>The Tooling API's own way, {@code GradleConnector.disconnect()}, does not do: it reaches
 * only the connections still open (the engine closes each after its build) and does nothing on
 * Gradle before 6.8. The registry and the idle time are set through the Tooling API's connector
 * ({@code daemonBaseDir}, {@code daemonMaxIdleTime}), which every Gradle version honours; the
 * same {@code -Dorg.gradle.daemon.*} arguments are ignored by Gradle 5.
 */
final class Daemons {
    /** How long one of the engine's daemons may stay idle. */
    static final int IDLE_MINUTES = 15;

    /** The class every Gradle daemon runs, on its command line. */
    private static final String DAEMON_MAIN = "org.gradle.launcher.daemon.bootstrap.GradleDaemon";

    /** Each daemon logs to {@code <registry>/<Gradle version>/daemon-<pid>.out.log}. */
    private static final Pattern LOG = Pattern.compile("daemon-(\\d+)\\.out\\.log");

    private Daemons() {
    }

    /** The daemon registry of the engine's daemons for a project folder. */
    static File registry(Path cacheDir, File projectDir) {
        String hash = Sync.sha256(projectDir.getAbsolutePath().getBytes(StandardCharsets.UTF_8)).substring(0, 16);
        return cacheDir.resolve("daemons").resolve(hash).toFile();
    }

    /** Makes the connector's builds run on the daemons of {@code registry}, which idle out early. */
    static void configure(GradleConnector connector, File registry) {
        if (connector instanceof DefaultGradleConnector c) {
            c.daemonBaseDir(registry);
            c.daemonMaxIdleTime(IDLE_MINUTES, TimeUnit.MINUTES);
        }
    }

    /**
     * Stops every daemon of {@code registry} that still runs: asks each to end (SIGTERM, which a
     * daemon answers in well under a second) and kills those still there after
     * {@code graceMillis}. Returns how many it stopped.
     */
    static int stop(File registry, long graceMillis, PrintStream log) {
        List<ProcessHandle> daemons = running(registry.toPath());
        if (daemons.isEmpty()) {
            return 0;
        }
        List<CompletableFuture<ProcessHandle>> exits = new ArrayList<>();
        for (ProcessHandle daemon : daemons) {
            daemon.destroy();
            exits.add(daemon.onExit());
        }
        try {
            CompletableFuture.allOf(exits.toArray(new CompletableFuture<?>[0])).get(graceMillis, TimeUnit.MILLISECONDS);
        } catch (Exception e) {
            for (ProcessHandle daemon : daemons) {
                if (daemon.isAlive()) {
                    daemon.destroyForcibly();
                }
            }
        }
        if (log != null) {
            log.println("snout-engine: stopped " + daemons.size() + " Gradle daemon(s)");
        }
        return daemons.size();
    }

    /**
     * The daemons of the registry that run: the processes its logs name, when they still run
     * Gradle's daemon and started before their log was last written (so not a later process
     * that got the pid of a daemon gone since: a daemon writes to its log as it stops).
     */
    static List<ProcessHandle> running(Path registry) {
        List<ProcessHandle> out = new ArrayList<>();
        if (!Files.isDirectory(registry)) {
            return out;
        }
        try (DirectoryStream<Path> versions = Files.newDirectoryStream(registry, Files::isDirectory)) {
            for (Path version : versions) {
                try (DirectoryStream<Path> logs = Files.newDirectoryStream(version, "daemon-*.out.log")) {
                    for (Path file : logs) {
                        Matcher m = LOG.matcher(file.getFileName().toString());
                        if (m.matches()) {
                            daemon(Long.parseLong(m.group(1)), file).ifPresent(out::add);
                        }
                    }
                }
            }
        } catch (IOException | RuntimeException e) {
            // a registry being cleaned up: stop what was found
        }
        return out;
    }

    private static Optional<ProcessHandle> daemon(long pid, Path logFile) throws IOException {
        ProcessHandle process = ProcessHandle.of(pid).orElse(null);
        if (process == null || !process.isAlive()) {
            return Optional.empty();
        }
        ProcessHandle.Info info = process.info();
        String command = info.commandLine().orElse("");
        Instant started = info.startInstant().orElse(null);
        // The last write, not the creation: macOS gives the creation time to the second only.
        Instant logged = Files.getLastModifiedTime(logFile).toInstant();
        if (!command.contains(DAEMON_MAIN) || started == null || started.isAfter(logged)) {
            return Optional.empty();
        }
        return Optional.of(process);
    }
}
