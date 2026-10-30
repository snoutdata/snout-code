package com.snoutcode.engine.cli;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.gradle.tooling.BuildCancelledException;
import org.gradle.tooling.BuildLauncher;
import org.gradle.tooling.CancellationTokenSource;
import org.gradle.tooling.GradleConnectionException;
import org.gradle.tooling.GradleConnector;
import org.gradle.tooling.ProjectConnection;
import org.gradle.tooling.events.FailureResult;
import org.gradle.tooling.events.FinishEvent;
import org.gradle.tooling.events.OperationResult;
import org.gradle.tooling.events.OperationType;
import org.gradle.tooling.events.ProgressEvent;
import org.gradle.tooling.events.StartEvent;
import org.gradle.tooling.events.task.TaskFinishEvent;
import org.gradle.tooling.events.task.TaskOperationDescriptor;
import org.gradle.tooling.events.task.TaskSkippedResult;
import org.gradle.tooling.events.task.TaskSuccessResult;

import java.io.IOException;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.EnumSet;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.function.Consumer;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * Runs Gradle tasks through the Tooling API for the app, streaming what happens as LSP
 * notifications: {@code snout/taskOutput} (each line of standard output and error),
 * {@code snout/taskProgress} (each task started and finished, with its outcome),
 * {@code snout/testEvent} (each test suite and test started, finished or printing),
 * {@code snout/buildProblems} (compiler problems read from the output) and, last,
 * {@code snout/taskFinished}.
 *
 * <p>Every run carries the build directory init script, so its outputs stay out of the
 * repository, and the run init script ({@link #runInitScript}): a test listener that prints each
 * test event as a marked JSON line, which this class takes out of the output, plus the test
 * filters and debug agents a run asks for.
 */
final class GradleRuns {
    /** Where notifications go. */
    interface Client {
        void notify(String method, JsonObject params);
    }

    /** One running build or process, cancellable. */
    static final class Run {
        final String id;
        final long startNanos = System.nanoTime();
        final CancellationTokenSource token = GradleConnector.newCancellationTokenSource();
        volatile Process process;
        volatile boolean cancelled;
        /** Tests that failed in this run: {@code class} and {@code method}. */
        final List<String[]> failedTests = new ArrayList<>();
        int passed;
        int failed;
        int skipped;

        Run(String id) {
            this.id = id;
        }

        void cancel() {
            cancelled = true;
            token.cancel();
            Process p = process;
            if (p != null) {
                p.descendants().forEach(ProcessHandle::destroy);
                p.destroy();
            }
        }

        long millis() {
            return (System.nanoTime() - startNanos) / 1_000_000;
        }
    }

    /** What to run. */
    static final class Invocation {
        Sync.Options options;
        final List<String> tasks = new ArrayList<>();
        /** Arguments after the tasks: task options such as {@code --tests}, {@code -P}, {@code --info}. */
        final List<String> args = new ArrayList<>();
        /** Test tasks run even when up to date, so a rerun reports its tests again. */
        boolean rerunTests;
        /** Test filters ({@code --tests} patterns) for the test tasks of {@link #filterTasks}. */
        final List<String> testFilters = new ArrayList<>();
        final List<String> filterTasks = new ArrayList<>();
        /** JavaExec and Test tasks to start with a JDWP agent listening on {@link #debugPort}. */
        final List<String> debugTasks = new ArrayList<>();
        int debugPort;
        /** Sees every output line, test event lines excluded. */
        Consumer<String> lineHook;
        /**
         * Environment variables for the build, on top of the engine's own: the tasks' forked
         * JVMs (tests, {@code run}, {@code bootRun}) inherit them.
         */
        final Map<String, String> env = new LinkedHashMap<>();
    }

    static final String TEST_EVENT_MARKER = "@@snout-test ";
    static final Pattern JDWP_LISTENING = Pattern.compile("Listening for transport dt_socket at address: (?:[^:\\s]*:)?(\\d+)");

    private GradleRuns() {
    }

    /**
     * Runs the invocation to its end and sends the notifications on the way. Returns the
     * {@code snout/taskFinished} parameters, which it has also sent.
     */
    static JsonObject execute(Invocation inv, Run run, Client client) {
        BuildProblems stdoutProblems = new BuildProblems();
        BuildProblems stderrProblems = new BuildProblems();
        String outcome = "success";
        String error = null;
        LineOutput stdout = new LineOutput(line -> line(inv, run, client, "stdout", line, stdoutProblems));
        LineOutput stderr = new LineOutput(line -> line(inv, run, client, "stderr", line, stderrProblems));
        try (ProjectConnection connection = Sync.connector(inv.options.projectDir, inv.options).connect()) {
            List<String> args = Sync.arguments(initScripts(inv.options), inv.options.projectCacheDir, inv.options);
            if (inv.rerunTests) {
                args.add("-Psnout.rerunTests=true");
            }
            if (!inv.testFilters.isEmpty()) {
                args.add("-Psnout.testFilter=" + String.join("|", inv.testFilters));
                args.add("-Psnout.testTasks=" + String.join("|", inv.filterTasks));
            }
            if (!inv.debugTasks.isEmpty()) {
                args.add("-Psnout.debugTasks=" + String.join("|", inv.debugTasks));
                args.add("-Psnout.debugPort=" + inv.debugPort);
            }
            args.addAll(inv.args);
            BuildLauncher launcher = connection.newBuild()
                .forTasks(inv.tasks.toArray(new String[0]))
                .withArguments(args)
                .withCancellationToken(run.token.token())
                .setStandardOutput(stdout)
                .setStandardError(stderr)
                .setColorOutput(false);
            if (inv.options.javaHome != null) {
                launcher.setJavaHome(inv.options.javaHome);
            }
            if (!inv.env.isEmpty()) {
                // The Tooling API replaces the whole environment: keep the engine's own under the run's.
                Map<String, String> env = new HashMap<>(System.getenv());
                env.putAll(inv.env);
                launcher.setEnvironmentVariables(env);
            }
            launcher.addProgressListener(event -> progress(run, client, event), EnumSet.of(OperationType.TASK));
            launcher.run();
        } catch (BuildCancelledException e) {
            outcome = "cancelled";
        } catch (GradleConnectionException | IOException | IllegalStateException e) {
            outcome = run.cancelled ? "cancelled" : "failed";
            error = SnoutEngine.describe(e);
        } finally {
            stdout.close();
            stderr.close();
        }
        List<BuildProblems.Problem> problems = new ArrayList<>(stderrProblems.finish());
        problems.addAll(stdoutProblems.finish());
        JsonObject p = new JsonObject();
        p.addProperty("runId", run.id);
        p.add("problems", BuildProblems.toJson(dedupe(problems)));
        client.notify("snout/buildProblems", p);
        return finish(run, client, outcome, error, null);
    }

    /** Sends {@code snout/taskFinished} and returns its parameters. */
    static JsonObject finish(Run run, Client client, String outcome, String error, Integer exitCode) {
        JsonObject done = new JsonObject();
        done.addProperty("runId", run.id);
        done.addProperty("outcome", outcome);
        done.addProperty("millis", run.millis());
        if (exitCode != null) {
            done.addProperty("exitCode", exitCode);
        }
        if (error != null) {
            done.addProperty("error", error);
        }
        if (run.passed + run.failed + run.skipped > 0) {
            JsonObject tests = new JsonObject();
            tests.addProperty("passed", run.passed);
            tests.addProperty("failed", run.failed);
            tests.addProperty("skipped", run.skipped);
            done.add("tests", tests);
        }
        client.notify("snout/taskFinished", done);
        return done;
    }

    private static List<BuildProblems.Problem> dedupe(List<BuildProblems.Problem> problems) {
        Set<BuildProblems.Problem> seen = new LinkedHashSet<>(problems);
        return new ArrayList<>(seen);
    }

    private static void line(Invocation inv, Run run, GradleRuns.Client client, String stream, String line, BuildProblems problems) {
        int marker = line.indexOf(TEST_EVENT_MARKER);
        if (marker >= 0) {
            testEvent(run, client, line.substring(marker + TEST_EVENT_MARKER.length()));
            return;
        }
        problems.accept(line);
        if (inv.lineHook != null) {
            inv.lineHook.accept(line);
        }
        output(run, client, stream, line);
    }

    static void output(Run run, Client client, String stream, String line) {
        JsonObject p = new JsonObject();
        p.addProperty("runId", run.id);
        p.addProperty("stream", stream);
        p.addProperty("line", line);
        client.notify("snout/taskOutput", p);
    }

    static void testEvent(Run run, Client client, String json) {
        JsonObject event;
        try {
            event = JsonParser.parseString(json).getAsJsonObject();
        } catch (RuntimeException e) {
            return;
        }
        if ("finished".equals(string(event, "event")) && "test".equals(string(event, "kind"))) {
            switch (String.valueOf(string(event, "outcome"))) {
                case "passed" -> run.passed++;
                case "failed" -> {
                    run.failed++;
                    run.failedTests.add(new String[] {string(event, "className"), methodName(string(event, "name"))});
                }
                case "skipped" -> run.skipped++;
                default -> {
                }
            }
        }
        event.addProperty("runId", run.id);
        client.notify("snout/testEvent", event);
    }

    /** A test's method name from its descriptor name: {@code add(int, int)[2]} is {@code add}. */
    static String methodName(String name) {
        if (name == null) {
            return null;
        }
        int cut = name.length();
        for (char c : new char[] {'(', '['}) {
            int i = name.indexOf(c);
            if (i > 0) {
                cut = Math.min(cut, i);
            }
        }
        return name.substring(0, cut).trim();
    }

    private static void progress(Run run, Client client, ProgressEvent event) {
        if (!(event.getDescriptor() instanceof TaskOperationDescriptor task)) {
            return;
        }
        JsonObject p = new JsonObject();
        p.addProperty("runId", run.id);
        p.addProperty("task", task.getTaskPath());
        if (event instanceof StartEvent) {
            p.addProperty("event", "started");
        } else if (event instanceof FinishEvent finish) {
            p.addProperty("event", "finished");
            OperationResult result = finish.getResult();
            p.addProperty("outcome", outcome(result));
            p.addProperty("millis", result.getEndTime() - result.getStartTime());
            if (result instanceof FailureResult failure && !failure.getFailures().isEmpty()) {
                p.addProperty("failure", failure.getFailures().get(0).getMessage());
            }
        } else {
            return;
        }
        client.notify("snout/taskProgress", p);
    }

    private static String outcome(OperationResult result) {
        if (result instanceof TaskSkippedResult skipped) {
            String why = String.valueOf(skipped.getSkipMessage());
            return why.contains("NO-SOURCE") ? "no-source" : "skipped";
        }
        if (result instanceof TaskSuccessResult success) {
            if (success.isFromCache()) {
                return "from-cache";
            }
            return success.isUpToDate() ? "up-to-date" : "success";
        }
        if (result instanceof FailureResult) {
            return "failed";
        }
        return "success";
    }

    /** The port a JVM's JDWP agent listens on, from the line it prints; -1 if this is not that line. */
    static int jdwpPort(String line) {
        Matcher m = JDWP_LISTENING.matcher(line);
        return m.find() ? Integer.parseInt(m.group(1)) : -1;
    }

    /**
     * The run init script. Applied to every build of the composite, it:
     *
     * <ul>
     *   <li>adds a test listener to every Test task that prints each event as a JSON line after
     *       {@value #TEST_EVENT_MARKER}: suites and tests started and finished, with the outcome,
     *       duration, failure message and stack trace, and each test's output;</li>
     *   <li>with {@code -Psnout.rerunTests}, makes test tasks never up to date;</li>
     *   <li>with {@code -Psnout.testFilter=p1|p2} and {@code -Psnout.testTasks=:a:test|...},
     *       filters those tasks as {@code --tests p1 --tests p2} would;</li>
     *   <li>with {@code -Psnout.debugTasks=:a:run|...} and {@code -Psnout.debugPort=n}, starts the
     *       JVM of those JavaExec or Test tasks with a JDWP agent that waits on port n.</li>
     * </ul>
     *
     * Task paths are identity paths: {@code :included:app:test} for a task of an included build.
     */
    /**
     * The init scripts of every Gradle run: build directories out of the repository, room for the
     * Scala compiler (as the Scala bridge gives it: a large joint compile runs out of Gradle's
     * default heap in {@code :classes} too), and the run script.
     */
    static List<Path> initScripts(Sync.Options options) throws IOException {
        List<Path> scripts = new ArrayList<>();
        if (options.buildRoot != null) {
            scripts.add(Sync.buildDirInitScript(options.buildRoot));
        }
        scripts.add(ScalaBridge.compilerMemoryInitScript());
        scripts.add(runInitScript());
        return scripts;
    }

    static Path runInitScript() throws IOException {
        String text = """
            // Generated by snout-engine: test events, test filters and debug agents for SnoutCode.
            import groovy.json.JsonOutput

            class SnoutTestEvents implements TestListener, TestOutputListener {
                private final String task

                SnoutTestEvents(String task) {
                    this.task = task
                }

                private static String id(TestDescriptor d) {
                    if (d == null) {
                        return null
                    }
                    try {
                        def i = d.id
                        if (i != null) {
                            return i.toString()
                        }
                    } catch (Throwable ignored) {
                    }
                    return d.className ? d.className + (d.composite ? '' : '#' + d.name) : d.name
                }

                private static String displayName(TestDescriptor d) {
                    try {
                        return d.displayName
                    } catch (Throwable ignored) {
                        return d.name
                    }
                }

                private Map describe(String event, TestDescriptor d) {
                    [event: event, kind: d.composite ? 'suite' : 'test', id: id(d), parent: id(d.parent),
                     name: d.name, displayName: displayName(d), className: d.className, task: task]
                }

                private static void emit(Map event) {
                    System.out.println('@@snout-test ' + JsonOutput.toJson(event))
                }

                void beforeSuite(TestDescriptor suite) {
                    emit(describe('started', suite))
                }

                void afterSuite(TestDescriptor suite, TestResult result) {
                    emit(finished(suite, result))
                }

                void beforeTest(TestDescriptor test) {
                    emit(describe('started', test))
                }

                void afterTest(TestDescriptor test, TestResult result) {
                    emit(finished(test, result))
                }

                void onOutput(TestDescriptor test, TestOutputEvent output) {
                    def e = describe('output', test)
                    e.destination = output.destination.toString() == 'StdErr' ? 'stderr' : 'stdout'
                    e.text = output.message
                    emit(e)
                }

                private Map finished(TestDescriptor d, TestResult r) {
                    def e = describe('finished', d)
                    def type = r.resultType.toString()
                    e.outcome = type == 'SUCCESS' ? 'passed' : type == 'FAILURE' ? 'failed' : 'skipped'
                    e.durationMillis = r.endTime - r.startTime
                    if (d.composite) {
                        e.tests = r.testCount
                        e.failedTests = r.failedTestCount
                        e.skippedTests = r.skippedTestCount
                    }
                    def failures = r.exceptions
                    if (failures) {
                        def first = failures[0]
                        e.message = first.message ?: first.toString()
                        e.exception = first.class.name
                        def text = new StringWriter()
                        first.printStackTrace(new PrintWriter(text))
                        e.stackTrace = text.toString()
                    }
                    e
                }
            }

            def snoutProps = gradle.startParameter.projectProperties
            def snoutRerun = snoutProps.containsKey('snout.rerunTests')
            def snoutSplit = { String key -> (snoutProps[key] ?: '').split('\\\\|').findAll { it } }
            def snoutFilters = snoutSplit('snout.testFilter')
            def snoutFilterTasks = snoutSplit('snout.testTasks')
            def snoutDebugTasks = snoutSplit('snout.debugTasks')
            def snoutDebugPort = snoutProps['snout.debugPort']

            def snoutPath = { task ->
                try {
                    return task.identityPath.toString()
                } catch (Throwable ignored) {
                    return task.path
                }
            }

            gradle.allprojects { p ->
                def configure = { t ->
                    String path = snoutPath(t)
                    if (t instanceof Test) {
                        def listener = new SnoutTestEvents(path)
                        t.addTestListener(listener)
                        t.addTestOutputListener(listener)
                        if (snoutRerun) {
                            t.outputs.upToDateWhen { false }
                        }
                        if (snoutFilters && snoutFilterTasks.contains(path)) {
                            if (t.filter.metaClass.respondsTo(t.filter, 'setCommandLineIncludePatterns', Collection)) {
                                t.filter.commandLineIncludePatterns = snoutFilters
                            } else {
                                snoutFilters.each { t.filter.includeTestsMatching(it) }
                            }
                        }
                    }
                    if (snoutDebugTasks.contains(path) && (t instanceof JavaExec || t instanceof Test)) {
                        t.jvmArgs("-agentlib:jdwp=transport=dt_socket,server=y,suspend=y,address=${snoutDebugPort}")
                        if (t instanceof Test) {
                            t.maxParallelForks = 1
                            t.forkEvery = 0
                        }
                    }
                }
                [Test, JavaExec].each { type ->
                    def tasks = p.tasks.withType(type)
                    if (tasks.metaClass.respondsTo(tasks, 'configureEach', Action)) {
                        tasks.configureEach(configure as Action)
                    } else {
                        tasks.all(configure)
                    }
                }
            }
            """;
        return Sync.writeScript("snout-run-init", text);
    }

    private static String string(JsonObject o, String key) {
        JsonElement e = o.get(key);
        return e == null || e.isJsonNull() ? null : e.getAsString();
    }

    /** The arguments a JSON array of strings holds. */
    /** An {@code env} object of names to values; values that are not strings are skipped. */
    static Map<String, String> env(JsonObject o) {
        Map<String, String> out = new LinkedHashMap<>();
        JsonElement env = o == null ? null : o.get("env");
        if (env != null && env.isJsonObject()) {
            for (Map.Entry<String, JsonElement> e : env.getAsJsonObject().entrySet()) {
                if (e.getValue().isJsonPrimitive()) {
                    out.put(e.getKey(), e.getValue().getAsString());
                }
            }
        }
        return out;
    }

    static List<String> strings(JsonArray array) {
        List<String> out = new ArrayList<>();
        if (array != null) {
            array.forEach(e -> out.add(e.getAsString()));
        }
        return out;
    }
}
