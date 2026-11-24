package com.snoutcode.engine.cli;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;

import java.util.ArrayList;
import java.util.EnumSet;
import java.util.List;
import java.util.function.Consumer;
import java.util.function.Supplier;

import org.gradle.tooling.LongRunningOperation;
import org.gradle.tooling.events.OperationType;
import org.gradle.tooling.events.ProgressEvent;
import org.gradle.tooling.events.StartEvent;

/**
 * What the engine's own Gradle runs do right now (sync, source generators, the Scala compile),
 * told to the app as {@code snout/activity} notifications, so a first sync that downloads every
 * dependency for minutes shows it working: {@code {"lines": ["Download https://...", "Task
 * :core:compileScala"]}}. The lines are batched, a few times a second at most. Once no run of
 * ours is left, a batch says {@code "idle": true}: the last line is no longer going on.
 *
 * <p>One engine serves one project, so the sink is the process's own; without one (the
 * command line, tests) nothing is sent.
 */
final class Activity {
    /** How often, at most, a batch goes out. */
    static final long PERIOD_MILLIS = 300;
    /** The most lines in one batch: older ones are counted, not sent. */
    static final int MAX_LINES = 100;

    private static final Object LOCK = new Object();
    private static Consumer<JsonObject> toApp;
    private static List<String> pending = new ArrayList<>();
    private static int dropped;
    /** The Gradle runs going on now, and whether the last one ended since the last batch. */
    private static int running;
    private static boolean idle;
    private static Thread flusher;

    private Activity() {}

    /** Send the activity to the app from now on. */
    static void sendTo(Consumer<JsonObject> sink) {
        synchronized (LOCK) {
            sink(sink);
            if (flusher == null) {
                flusher = new Thread(Activity::flushLoop, "snout-activity");
                flusher.setDaemon(true);
                flusher.start();
            }
        }
    }

    /** Send the activity to {@code sink} when {@link #flush} runs (no batching thread: for tests). */
    static void sink(Consumer<JsonObject> sink) {
        synchronized (LOCK) {
            toApp = sink;
            pending = new ArrayList<>();
            dropped = 0;
            idle = false;
        }
    }

    /** Watch a Gradle run: its downloads, the projects it configures and the tasks it runs. */
    static <T extends LongRunningOperation> T watch(T operation) {
        operation.addProgressListener(
            Activity::on, EnumSet.of(OperationType.FILE_DOWNLOAD, OperationType.PROJECT_CONFIGURATION, OperationType.TASK));
        return operation;
    }

    /** Run a watched Gradle run: when it was the last one going on, the app hears it is over. */
    static <R> R running(Supplier<R> run) {
        synchronized (LOCK) {
            running++;
            idle = false;
        }
        try {
            return run.get();
        } finally {
            synchronized (LOCK) {
                running--;
                idle = running == 0;
            }
        }
    }

    /** {@link #running} for a run without a result. */
    static void runningVoid(Runnable run) {
        running(() -> {
            run.run();
            return null;
        });
    }

    private static void on(ProgressEvent event) {
        if (event instanceof StartEvent) {
            say(event.getDescriptor().getDisplayName());
        }
    }

    /** One line of activity. */
    static void say(String line) {
        if (line == null || line.isBlank()) {
            return;
        }
        synchronized (LOCK) {
            if (toApp == null) {
                return;
            }
            // The newest lines are kept: the last one is what the app shows as going on now.
            if (pending.size() >= MAX_LINES) {
                pending.remove(0);
                dropped++;
            }
            pending.add(line.strip());
        }
    }

    private static void flushLoop() {
        while (true) {
            try {
                Thread.sleep(PERIOD_MILLIS);
            } catch (InterruptedException e) {
                return;
            }
            flush();
        }
    }

    /** Send what came since the last batch, if anything did. */
    static void flush() {
        List<String> lines;
        int more;
        boolean over;
        Consumer<JsonObject> sink;
        synchronized (LOCK) {
            if ((pending.isEmpty() && !idle) || toApp == null) {
                return;
            }
            lines = pending;
            more = dropped;
            over = idle;
            sink = toApp;
            pending = new ArrayList<>();
            dropped = 0;
            idle = false;
        }
        JsonObject p = batch(lines, more);
        if (over) {
            p.addProperty("idle", true);
        }
        sink.accept(Rpc.notification("snout/activity", p));
    }

    /** The notification's parameters: the lines, and how many more were left out. */
    static JsonObject batch(List<String> lines, int more) {
        JsonObject p = new JsonObject();
        JsonArray list = new JsonArray();
        lines.forEach(list::add);
        p.add("lines", list);
        if (more > 0) {
            p.addProperty("more", more);
        }
        return p;
    }
}
