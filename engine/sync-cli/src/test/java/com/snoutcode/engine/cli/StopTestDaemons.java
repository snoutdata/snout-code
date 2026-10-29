package com.snoutcode.engine.cli;

import org.junit.platform.launcher.LauncherSession;
import org.junit.platform.launcher.LauncherSessionListener;

import java.io.File;

/**
 * Stops the Gradle daemons the tests started, when the test run ends.
 *
 * <p>The test task points {@link Daemons#FALLBACK_REGISTRY} at a registry of the tests' own, so
 * every build a test runs without a registry of its own lands there (with the engine's short
 * idle time) instead of in the user's {@code ~/.gradle/daemon}. Tests that pick a registry stop
 * its daemons themselves.
 */
public final class StopTestDaemons implements LauncherSessionListener {
    @Override
    public void launcherSessionClosed(LauncherSession session) {
        File registry = Daemons.fallbackRegistry();
        if (registry != null) {
            Daemons.stop(registry, Lsp.DAEMON_GRACE_MILLIS, System.err);
        }
    }
}
