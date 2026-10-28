package com.snoutcode.engine.cli;

import org.junit.jupiter.api.Test;

import java.io.ByteArrayOutputStream;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Optional;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/** A jdtls left on the workspace by an engine that is gone is stopped; others are not touched. */
class LeftoversTest {
    @Test
    void aProcessOnTheWorkspaceWhoseEngineIsGoneIsStopped() throws Exception {
        Path scratch = Path.of(System.getProperty("snout.test.scratch"));
        Path workspace = scratch.resolve("leftovers-" + System.nanoTime()).resolve("workspace");
        Files.createDirectories(workspace);
        Path other = workspace.resolveSibling("other-workspace");
        // A stand-in for jdtls whose parent ends at once: the shell that started it in the
        // background exits, so the system's init takes it over. `; :` keeps the shell from
        // replacing itself with sleep, so its command line keeps the -data argument.
        String marker = "leftovers-" + System.nanoTime();
        start("sh", "-c", "sh -c 'sleep 60; :' " + marker + " -data '" + workspace + "' & exit 0");
        start("sh", "-c", "sh -c 'sleep 60; :' " + marker + " -data '" + other + "' & exit 0");
        ProcessHandle onWorkspace = find(marker, workspace.toString());
        ProcessHandle onOther = find(marker, other.toString());
        try {
            ByteArrayOutputStream bytes = new ByteArrayOutputStream();
            PrintStream log = new PrintStream(bytes, true, StandardCharsets.UTF_8);
            assertEquals(1, Leftovers.stopJdtls(workspace, log));
            assertFalse(onWorkspace.isAlive(), "the leftover on the workspace is stopped");
            assertTrue(onOther.isAlive(), "a process on another workspace is left alone");
            assertTrue(bytes.toString(StandardCharsets.UTF_8).contains("an engine that is gone"));
            assertEquals(0, Leftovers.stopJdtls(workspace, log));
        } finally {
            onWorkspace.destroyForcibly();
            onOther.destroyForcibly();
        }
    }

    private static void start(String... command) throws Exception {
        Process p = new ProcessBuilder(command).redirectErrorStream(true).start();
        p.getInputStream().close();
        assertEquals(0, p.waitFor());
    }

    /** The process whose arguments hold {@code marker} and {@code workspace}, once it shows. */
    private static ProcessHandle find(String marker, String workspace) throws InterruptedException {
        for (int i = 0; i < 100; i++) {
            Optional<ProcessHandle> found;
            try (var all = ProcessHandle.allProcesses()) {
                found = all.filter(p -> Leftovers.usesWorkspace(p, workspace)
                    && String.join(" ", p.info().arguments().orElse(new String[0])).contains(marker)).findFirst();
            }
            long self = ProcessHandle.current().pid();
            if (found.isPresent() && found.get().parent().map(ProcessHandle::pid).orElse(1L) != self) {
                return found.get();
            }
            Thread.sleep(50);
        }
        throw new AssertionError("the stand-in did not start");
    }
}
