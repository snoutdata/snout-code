package com.snoutcode.engine.cli;

import java.io.PrintStream;
import java.nio.file.Path;
import java.util.List;
import java.util.Optional;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.TimeUnit;

/**
 * What an earlier engine of the same project left running: a jdtls whose engine is gone holds
 * the lock of its workspace, so the jdtls this engine starts there could not open it.
 *
 * <p>A jdtls counts when its command line has {@code -data <workspace>} and it is not this
 * engine's own: either its engine is gone (no parent, or the system's init took it over), or
 * its engine was started by the same app as this one (the app replacing an engine that is
 * still stopping), which gets a few seconds to end with it first. A jdtls of an engine another
 * app started is never touched.
 */
final class Leftovers {
    /** How long a jdtls whose engine is stopping gets to end on its own. */
    private static final long REPLACED_GRACE_MILLIS = 5000;

    /** How long a jdtls asked to end (SIGTERM) gets before it is killed. */
    private static final long STOP_GRACE_MILLIS = 5000;

    private Leftovers() {
    }

    /** Stops the jdtls processes left on {@code workspace}; returns how many it stopped. */
    static int stopJdtls(Path workspace, PrintStream log) {
        String ws = workspace.toAbsolutePath().toString();
        long self = ProcessHandle.current().pid();
        long app = ProcessHandle.current().parent().map(ProcessHandle::pid).orElse(-1L);
        List<ProcessHandle> found;
        try (var all = ProcessHandle.allProcesses()) {
            found = all.filter(p -> p.pid() != self && usesWorkspace(p, ws)).toList();
        }
        int stopped = 0;
        for (ProcessHandle jdtls : found) {
            Optional<ProcessHandle> engine = jdtls.parent().filter(p -> p.pid() > 1 && p.isAlive() && isEngine(p));
            boolean orphan = engine.isEmpty();
            boolean replaced = engine.isPresent() && engine.get().parent().map(ProcessHandle::pid).orElse(-2L) == app;
            if (!orphan && !replaced) {
                log.println("snout-engine: jdtls (pid " + jdtls.pid() + ") of another engine uses this workspace; leaving it");
                continue;
            }
            if (replaced && waitFor(jdtls, REPLACED_GRACE_MILLIS)) {
                continue;
            }
            jdtls.destroy();
            if (!waitFor(jdtls, STOP_GRACE_MILLIS)) {
                jdtls.destroyForcibly();
                waitFor(jdtls, STOP_GRACE_MILLIS);
            }
            log.println("snout-engine: stopped jdtls (pid " + jdtls.pid() + ") left on this workspace by "
                + (orphan ? "an engine that is gone" : "the engine this one replaces"));
            stopped++;
        }
        return stopped;
    }

    /** Whether a process runs with {@code -data <workspace>}: a jdtls on that workspace. */
    static boolean usesWorkspace(ProcessHandle p, String workspace) {
        String[] args = p.info().arguments().orElse(null);
        if (args == null) {
            return false;
        }
        for (int i = 0; i + 1 < args.length; i++) {
            if ("-data".equals(args[i]) && workspace.equals(args[i + 1])) {
                return true;
            }
        }
        return false;
    }

    /** Whether {@code p} runs snout-engine. */
    private static boolean isEngine(ProcessHandle p) {
        String[] args = p.info().arguments().orElse(new String[0]);
        for (String a : args) {
            if (a.equals(SnoutEngine.class.getName())) {
                return true;
            }
        }
        return false;
    }

    /** Waits for {@code p} to end; true when it did within {@code millis}. */
    private static boolean waitFor(ProcessHandle p, long millis) {
        if (!p.isAlive()) {
            return true;
        }
        CompletableFuture<ProcessHandle> exit = p.onExit();
        try {
            exit.get(millis, TimeUnit.MILLISECONDS);
            return true;
        } catch (Exception e) {
            return !p.isAlive();
        }
    }
}
