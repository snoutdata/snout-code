package com.snoutcode.engine.jdtls;

import org.eclipse.core.resources.IWorkspace;
import org.eclipse.core.resources.ResourcesPlugin;
import org.eclipse.core.runtime.NullProgressMonitor;

import java.util.concurrent.atomic.AtomicBoolean;

/**
 * Saves jdtls's workspace when its JVM ends on a signal. jdtls saves it on the LSP
 * {@code shutdown}, but the app's quit (and the engine's own end) stops jdtls with SIGTERM, and
 * Equinox then exits without saving: the next start finds "unsaved changes in the previous
 * session" and refreshes every linked folder to recover them, which on a large project made
 * jdtls take 4.2 s to be ready instead of 1.6 s.
 *
 * <p>The save runs in a JVM shutdown hook, bounded by {@link #LIMIT_MILLIS} (a build holding the
 * workspace lock must not keep jdtls alive: whoever stopped it kills it after a while anyway).
 * After a clean shutdown the workspace is closed and the hook does nothing.
 */
final class SaveOnExit {
    static final long LIMIT_MILLIS = 4000;

    private static final AtomicBoolean REGISTERED = new AtomicBoolean();

    private SaveOnExit() {
    }

    /** Registers the hook, once per JVM. */
    static void register() {
        if (!REGISTERED.compareAndSet(false, true)) {
            return;
        }
        Runtime.getRuntime().addShutdownHook(new Thread(SaveOnExit::save, "snout-save-workspace"));
    }

    private static void save() {
        Thread saver = new Thread(() -> {
            try {
                IWorkspace workspace = ResourcesPlugin.getWorkspace();
                workspace.save(true, new NullProgressMonitor());
            } catch (Throwable e) {
                // closed already (a clean shutdown), or the framework is going: nothing to save
            }
        }, "snout-save-workspace-run");
        saver.setDaemon(true);
        saver.start();
        try {
            saver.join(LIMIT_MILLIS);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        }
    }
}
