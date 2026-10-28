package com.snoutcode.engine.cli;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonNull;
import com.google.gson.JsonObject;
import com.snoutcode.engine.sync.action.TasksAction;
import org.gradle.tooling.BuildActionExecuter;
import org.gradle.tooling.ProjectConnection;

import java.io.IOException;
import java.io.PrintStream;
import java.net.URI;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.TreeMap;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicInteger;

/**
 * The engine's own LSP methods, all prefixed {@code snout/}: Gradle tasks, test runs, run
 * configurations, launching and debugging, and the incremental Scala bridge. {@link LspProxy}
 * routes them here; the README lists them with their parameters.
 *
 * <p>Requests run on a pool, so a long one (listing a big build's tasks) never holds up
 * another; runs answer at once with a {@code runId} and report through notifications.
 */
final class Engine implements GradleRuns.Client {
    /** The engine's line to jdtls, for the java-debug and importer commands. */
    interface Jdtls {
        CompletableFuture<JsonElement> request(String method, JsonElement params);

        /** Completes when jdtls has imported the projects (its {@code ServiceReady}). */
        CompletableFuture<Void> ready();
    }

    /** Where the engine's messages to the app go. */
    interface Out {
        void send(JsonObject message);
    }

    /** A request's failure, with an LSP error code. */
    static final class RequestError extends Exception {
        private static final long serialVersionUID = 1L;
        final int code;

        RequestError(int code, String message) {
            super(message);
            this.code = code;
        }

        static RequestError invalid(String message) {
            return new RequestError(-32602, message);
        }
    }

    private final JsonObject model;
    private final Map<String, ProjectSync.Plan> plans = new LinkedHashMap<>();
    private final Out out;
    private final PrintStream log;
    private final Path runDir;
    private volatile Jdtls jdtls;
    private volatile SpringSupport spring;
    private final ExecutorService pool = Executors.newCachedThreadPool(r -> {
        Thread t = new Thread(r, "snout-engine");
        t.setDaemon(true);
        return t;
    });
    private final ExecutorService scalaQueue = Executors.newSingleThreadExecutor(r -> {
        Thread t = new Thread(r, "snout-scala");
        t.setDaemon(true);
        return t;
    });
    private final Map<String, GradleRuns.Run> runs = new ConcurrentHashMap<>();
    private final Map<String, List<String[]>> lastFailedTests = new ConcurrentHashMap<>();
    private final Map<String, JsonObject> tasksCache = new ConcurrentHashMap<>();
    private final AtomicInteger runIds = new AtomicInteger();

    /**
     * @param model the synced model, with each build's entry under {@code gradleBuilds}
     * @param plans the builds, with the options their sync used (daemon JDK, caches, build root)
     * @param runDir where launches keep their argument files
     */
    Engine(JsonObject model, List<ProjectSync.Plan> plans, Out out, PrintStream log, Path runDir) {
        this.model = model;
        plans.forEach(p -> this.plans.put(p.id(), p));
        this.out = out;
        this.log = log;
        this.runDir = runDir;
    }

    void attach(Jdtls jdtls) {
        this.jdtls = jdtls;
    }

    void attach(SpringSupport spring) {
        this.spring = spring;
    }

    // ---- messages

    /** Takes a {@code snout/} request or notification from the app; answers on the pool. */
    void receive(JsonObject message) {
        pool.submit(() -> handle(message));
    }

    private void handle(JsonObject message) {
        String method = message.get("method").getAsString();
        JsonElement id = message.get("id");
        JsonObject params = message.has("params") && message.get("params").isJsonObject()
            ? message.getAsJsonObject("params") : new JsonObject();
        try {
            JsonElement result = dispatch(method, params, id == null);
            if (id != null) {
                out.send(Rpc.result(id, result == null ? JsonNull.INSTANCE : result));
            }
        } catch (RequestError e) {
            if (id != null) {
                out.send(Rpc.error(id, e.code, e.getMessage()));
            } else {
                log.println("snout-engine: " + method + ": " + e.getMessage());
            }
        } catch (Exception | LinkageError e) {
            String text = SnoutEngine.describe(e);
            log.println("snout-engine: " + method + " failed: " + text);
            if (id != null) {
                out.send(Rpc.error(id, -32603, text));
            }
        }
    }

    JsonElement dispatch(String method, JsonObject params, boolean notification) throws Exception {
        return switch (method) {
            case "snout/gradleTasks" -> gradleTasks(params);
            case "snout/runTask" -> runTask(params);
            case "snout/cancelTask" -> cancel(params);
            case "snout/runTests" -> runTests(params);
            case "snout/testsInFile" -> testsInFile(params);
            case "snout/runConfigurations" -> runConfigurations(params);
            case "snout/run" -> run(params);
            case "snout/scalaChanged" -> scalaChanged(params, notification);
            case "snout/springBeans" -> spring("snout.spring.beans", params);
            case "snout/springEndpoints" -> spring("snout.spring.endpoints", params);
            case "snout/springInjections" -> {
                JsonObject p = params.deepCopy();
                String file = string(params, "uri") != null ? string(params, "uri") : string(params, "path");
                if (file == null) {
                    throw RequestError.invalid("springInjections needs a uri");
                }
                p.addProperty("uri", file.startsWith("file:") ? file : toPath(file).toUri().toString());
                yield spring("snout.spring.injections", p);
            }
            default -> throw new RequestError(-32601, "unknown method " + method);
        };
    }

    private JsonElement spring(String command, JsonObject params) throws Exception {
        SpringSupport s = spring;
        if (s == null) {
            throw new RequestError(-32603, "Spring support is off");
        }
        return s.request(command, params);
    }

    @Override
    public void notify(String method, JsonObject params) {
        out.send(Rpc.notification(method, params));
    }

    /** Cancels every run and stops every process the engine started. */
    void shutdown() {
        runs.values().forEach(GradleRuns.Run::cancel);
        pool.shutdownNow();
        scalaQueue.shutdownNow();
    }

    // ---- snout/gradleTasks

    /**
     * {@code {build?, refresh?}}: the tasks of each build (or one), grouped per project as Gradle
     * groups them. Fetched once per build, then cached until {@code refresh}.
     */
    private JsonElement gradleTasks(JsonObject params) throws Exception {
        String only = string(params, "build");
        boolean refresh = params.has("refresh") && params.get("refresh").getAsBoolean();
        JsonArray builds = new JsonArray();
        List<ProjectSync.Plan> wanted = new ArrayList<>();
        for (ProjectSync.Plan plan : plans.values()) {
            if (only == null || only.equals(plan.id())) {
                wanted.add(plan);
            }
        }
        if (only != null && wanted.isEmpty()) {
            throw RequestError.invalid("no build " + only);
        }
        List<JsonObject> results = ProjectSync.lanes(wanted, Math.max(1, ProjectSync.defaultParallelism()), plan -> {
            if (refresh) {
                tasksCache.remove(plan.id());
            }
            return tasksCache.computeIfAbsent(plan.id(), id -> tasksOf(plan));
        });
        results.forEach(builds::add);
        JsonObject result = new JsonObject();
        result.add("builds", builds);
        return result;
    }

    private JsonObject tasksOf(ProjectSync.Plan plan) {
        long start = System.nanoTime();
        JsonObject out = new JsonObject();
        out.addProperty("build", plan.id());
        out.addProperty("rootDir", plan.rootDir().getAbsolutePath());
        JsonArray projects = new JsonArray();
        if (plan.needsJdk() != null) {
            out.addProperty("error", "Needs JDK " + plan.needsJdk() + ", which isn't installed");
            out.addProperty("needsJdk", plan.needsJdk());
            out.add("projects", projects);
            out.addProperty("millis", 0);
            return out;
        }
        try (ProjectConnection connection = Sync.connector(plan.rootDir(), plan.options()).connect()) {
            List<Path> scripts = new ArrayList<>();
            if (plan.options().buildRoot != null) {
                scripts.add(Sync.buildDirInitScript(plan.options().buildRoot));
            }
            scripts.add(GradleRuns.runInitScript());
            BuildActionExecuter<TasksAction.Result> action = connection.action(new TasksAction())
                .withArguments(Sync.arguments(scripts, plan.options().projectCacheDir, plan.options()));
            if (plan.options().javaHome != null) {
                action.setJavaHome(plan.options().javaHome);
            }
            TasksAction.Result result = action.run();
            JsonArray errors = new JsonArray();
            for (TasksAction.Build build : result.builds) {
                build.errors.forEach(errors::add);
                for (TasksAction.Project project : build.projects) {
                    projects.add(project(plan.id(), build, project));
                }
            }
            out.add("errors", errors);
        } catch (Exception | LinkageError e) {
            out.addProperty("error", SnoutEngine.describe(e));
        }
        out.add("projects", projects);
        out.addProperty("millis", (System.nanoTime() - start) / 1_000_000);
        return out;
    }

    private static JsonObject project(String buildId, TasksAction.Build build, TasksAction.Project project) {
        boolean included = "included".equals(build.kind);
        String prefix = included ? ":" + build.name : "";
        String path = ":".equals(project.path) ? (included ? prefix : ":") : prefix + project.path;
        JsonObject p = new JsonObject();
        p.addProperty("project", path);
        p.addProperty("name", project.name);
        p.addProperty("module", (included ? buildId + "/" + build.name : buildId) + (":".equals(project.path) ? "" : project.path));
        p.addProperty("projectDir", project.projectDir);
        Map<String, JsonArray> groups = new TreeMap<>(Comparator.comparing((String g) -> g.equals("other") ? 1 : 0)
            .thenComparing(g -> g.toLowerCase(Locale.ROOT)));
        for (TasksAction.Task task : project.tasks) {
            String group = task.group == null || task.group.isBlank() ? "other" : task.group;
            JsonObject t = new JsonObject();
            t.addProperty("name", task.name);
            t.addProperty("path", included ? prefix + task.path : task.path);
            t.addProperty("description", task.description);
            groups.computeIfAbsent(group, g -> new JsonArray()).add(t);
        }
        JsonArray out = new JsonArray();
        for (Map.Entry<String, JsonArray> e : groups.entrySet()) {
            List<JsonElement> tasks = new ArrayList<>();
            e.getValue().forEach(tasks::add);
            tasks.sort(Comparator.comparing(t -> t.getAsJsonObject().get("name").getAsString().toLowerCase(Locale.ROOT)));
            JsonArray sorted = new JsonArray();
            tasks.forEach(sorted::add);
            JsonObject g = new JsonObject();
            g.addProperty("name", e.getKey());
            g.add("tasks", sorted);
            out.add(g);
        }
        p.add("groups", out);
        return p;
    }

    // ---- snout/runTask and snout/cancelTask

    /**
     * {@code {build?, project?, task, args?}}: runs a task. {@code task} is a path
     * ({@code :app:compileJava}, {@code :included:app:test}), or a name in {@code project}.
     * Answers {@code {runId}} at once; the run reports through notifications.
     */
    private JsonElement runTask(JsonObject params) throws RequestError {
        ProjectSync.Plan plan = plan(string(params, "build"));
        String task = string(params, "task");
        if (task == null || task.isBlank()) {
            throw RequestError.invalid("runTask needs a task");
        }
        String project = string(params, "project");
        if (!task.startsWith(":") && project != null) {
            task = (project.equals(":") ? "" : project) + ":" + task;
        }
        GradleRuns.Invocation inv = new GradleRuns.Invocation();
        inv.options = plan.options();
        inv.tasks.add(task);
        inv.args.addAll(GradleRuns.strings(array(params, "args")));
        return start(inv, null);
    }

    /** Starts an invocation on the pool and answers {@code {runId}}. */
    private JsonObject start(GradleRuns.Invocation inv, String testModule) {
        return start(inv, testModule, newRun());
    }

    private JsonObject start(GradleRuns.Invocation inv, String testModule, GradleRuns.Run run) {
        pool.submit(() -> {
            try {
                GradleRuns.execute(inv, run, this);
                if (testModule != null) {
                    lastFailedTests.put(testModule, new ArrayList<>(run.failedTests));
                }
            } finally {
                runs.remove(run.id);
            }
        });
        JsonObject result = new JsonObject();
        result.addProperty("runId", run.id);
        JsonArray tasks = new JsonArray();
        inv.tasks.forEach(tasks::add);
        result.add("tasks", tasks);
        return result;
    }

    private GradleRuns.Run newRun() {
        GradleRuns.Run run = new GradleRuns.Run("run-" + runIds.incrementAndGet());
        runs.put(run.id, run);
        return run;
    }

    /** {@code {runId}}: cancels a run; a launched process is destroyed. */
    private JsonElement cancel(JsonObject params) {
        GradleRuns.Run run = runs.get(string(params, "runId"));
        if (run != null) {
            run.cancel();
        }
        JsonObject result = new JsonObject();
        result.addProperty("cancelled", run != null);
        return result;
    }

    // ---- snout/runTests

    /**
     * {@code {module, filter: {kind: all|package|class|method, package?, class?, method?},
     * rerunFailed?, task?, debug?}}: runs a module's tests, reporting {@code snout/testEvent}.
     */
    private JsonElement runTests(JsonObject params) throws Exception {
        Modules.Ref module = module(string(params, "module"));
        List<String> testTasks = module.testTasks();
        String task = string(params, "task");
        if (task != null) {
            // A path as the root build runs it, or a task name in the module.
            testTasks = List.of(task.startsWith(":") ? task
                : module.taskPath((":".equals(module.gradlePath()) ? "" : module.gradlePath()) + ":" + task));
        } else if (testTasks.size() > 1 && testTasks.stream().anyMatch(t -> t.endsWith(":test"))) {
            testTasks = testTasks.stream().filter(t -> t.endsWith(":test")).toList();
        }
        if (testTasks.isEmpty()) {
            throw RequestError.invalid("module " + module.id() + " has no test task");
        }
        GradleRuns.Invocation inv = new GradleRuns.Invocation();
        inv.options = plan(module.buildId()).options();
        inv.tasks.addAll(testTasks);
        inv.rerunTests = true;
        boolean rerunFailed = params.has("rerunFailed") && params.get("rerunFailed").getAsBoolean();
        if (rerunFailed) {
            List<String[]> failed = lastFailedTests.getOrDefault(module.id(), List.of());
            if (failed.isEmpty()) {
                throw RequestError.invalid("no failed tests to rerun in " + module.id());
            }
            for (String[] test : failed) {
                inv.testFilters.add(test[1] == null ? test[0] : test[0] + "." + test[1]);
            }
        } else {
            String pattern = testFilter(params.has("filter") && params.get("filter").isJsonObject()
                ? params.getAsJsonObject("filter") : new JsonObject());
            if (pattern != null) {
                inv.testFilters.add(pattern);
            }
        }
        inv.filterTasks.addAll(testTasks);
        if (params.has("debug") && params.get("debug").getAsBoolean()) {
            return debugGradle(inv, testTasks, module.id());
        }
        return start(inv, module.id());
    }

    /** The {@code --tests} pattern of a filter, or null for all tests. */
    static String testFilter(JsonObject filter) throws RequestError {
        String kind = filter.has("kind") ? filter.get("kind").getAsString() : "all";
        return switch (kind) {
            case "all" -> null;
            case "package" -> required(filter, "package") + ".*";
            case "class" -> required(filter, "class");
            case "method" -> required(filter, "class") + "." + required(filter, "method");
            default -> throw RequestError.invalid("unknown filter kind " + kind);
        };
    }

    // ---- snout/testsInFile

    /** {@code {uri | path, text?}}: the test classes and methods of a file, with ranges, and its module. */
    private JsonElement testsInFile(JsonObject params) throws IOException, RequestError {
        Path file = path(params);
        String text = params.has("text") ? params.get("text").getAsString() : Files.readString(file);
        String name = file.getFileName().toString();
        boolean scala = name.endsWith(".scala");
        JsonObject result = new JsonObject();
        result.addProperty("language", scala ? "scala" : "java");
        Modules.Ref owner = Modules.owner(model, file, true);
        result.addProperty("module", owner == null ? null : owner.id());
        result.add("classes", TestDiscovery.find(text, scala));
        return result;
    }

    // ---- snout/runConfigurations and snout/run

    /** {@code {module?}}: the run configurations the model implies. */
    private JsonElement runConfigurations(JsonObject params) {
        JsonObject result = new JsonObject();
        result.add("configurations", RunConfigurations.derive(model, string(params, "module")));
        return result;
    }

    /**
     * {@code {config, debug?, dryRun?}}: launches a run configuration (as
     * {@code snout/runConfigurations} lists it, or its {@code id}). Answers {@code {runId}}, with
     * {@code debug: {dapPort, attach}} in debug mode; {@code dryRun} resolves the launch
     * (main class, classpath, Java) and starts nothing.
     */
    private JsonElement run(JsonObject params) throws Exception {
        JsonObject config = config(params.get("config"));
        boolean debug = params.has("debug") && params.get("debug").getAsBoolean();
        boolean dryRun = params.has("dryRun") && params.get("dryRun").getAsBoolean();
        String kind = string(config, "kind");
        switch (String.valueOf(kind)) {
            case "main" -> {
                return Launches.main(this, config, debug, dryRun);
            }
            case "gradle" -> {
                GradleRuns.Invocation inv = new GradleRuns.Invocation();
                inv.options = plan(string(config, "build")).options();
                inv.tasks.add(string(config, "task"));
                inv.args.addAll(GradleRuns.strings(array(config, "args")));
                if (dryRun) {
                    return dryGradle(inv);
                }
                return debug ? debugGradle(inv, inv.tasks, null) : start(inv, null);
            }
            case "tests" -> {
                JsonObject tests = new JsonObject();
                tests.addProperty("module", string(config, "module"));
                tests.addProperty("task", string(config, "task"));
                tests.addProperty("debug", debug);
                if (dryRun) {
                    GradleRuns.Invocation inv = new GradleRuns.Invocation();
                    inv.options = plan(string(config, "build")).options();
                    inv.tasks.add(string(config, "task"));
                    return dryGradle(inv);
                }
                return runTests(tests);
            }
            default -> throw RequestError.invalid("unknown configuration kind " + kind);
        }
    }

    private JsonObject config(JsonElement config) throws RequestError {
        if (config != null && config.isJsonObject()) {
            return config.getAsJsonObject();
        }
        if (config != null && config.isJsonPrimitive()) {
            for (JsonElement c : RunConfigurations.derive(model, null)) {
                if (config.getAsString().equals(string(c.getAsJsonObject(), "id"))) {
                    return c.getAsJsonObject();
                }
            }
            throw RequestError.invalid("no run configuration " + config.getAsString());
        }
        throw RequestError.invalid("run needs a config");
    }

    private static JsonObject dryGradle(GradleRuns.Invocation inv) {
        JsonObject result = new JsonObject();
        result.addProperty("rootDir", inv.options.projectDir.getAbsolutePath());
        JsonArray tasks = new JsonArray();
        inv.tasks.forEach(tasks::add);
        result.add("tasks", tasks);
        JsonArray args = new JsonArray();
        inv.args.forEach(args::add);
        result.add("args", args);
        result.addProperty("javaHome", inv.options.javaHome == null ? null : inv.options.javaHome.getAbsolutePath());
        return result;
    }

    /**
     * Runs Gradle tasks whose JVM (JavaExec or Test) waits for a debugger, and opens a java-debug
     * session for it. Sends {@code snout/debugReady} once the JVM listens.
     */
    private JsonElement debugGradle(GradleRuns.Invocation inv, List<String> tasks, String testModule) throws Exception {
        int port = Launches.freePort();
        inv.debugTasks.addAll(tasks);
        inv.debugPort = port;
        int dapPort = Launches.startDebugSession(this);
        JsonObject attach = Launches.attachArguments(port, null);
        GradleRuns.Run run = newRun();
        AtomicBoolean ready = new AtomicBoolean();
        inv.lineHook = line -> {
            if (GradleRuns.jdwpPort(line) > 0 && ready.compareAndSet(false, true)) {
                JsonObject p = new JsonObject();
                p.addProperty("runId", run.id);
                p.addProperty("dapPort", dapPort);
                p.add("attach", attach);
                notify("snout/debugReady", p);
            }
        };
        JsonObject result = start(inv, testModule, run);
        JsonObject debug = new JsonObject();
        debug.addProperty("dapPort", dapPort);
        debug.add("attach", attach);
        result.add("debug", debug);
        return result;
    }

    // ---- snout/scalaChanged

    /**
     * {@code {files?}}: recompiles the Scala of the builds that hold these files (every build
     * with Scala when none are given) through Gradle's incremental Scala compile, refreshes the
     * class folders jdtls reads, and sends {@code snout/buildProblems} for the compile. As a
     * request it answers when done; as a notification it queues the compile.
     */
    private JsonElement scalaChanged(JsonObject params, boolean notification) throws Exception {
        List<Path> files = new ArrayList<>();
        for (JsonElement f : array(params, "files") == null ? new JsonArray() : array(params, "files")) {
            files.add(toPath(f.getAsString()));
        }
        CompletableFuture<JsonObject> done = new CompletableFuture<>();
        scalaQueue.submit(() -> {
            try {
                done.complete(compileScala(files));
            } catch (Exception | LinkageError e) {
                done.completeExceptionally(e);
            }
        });
        if (notification) {
            return null;
        }
        try {
            return done.get();
        } catch (ExecutionException e) {
            throw e.getCause() instanceof Exception x ? x : new IOException(e.getCause());
        }
    }

    private JsonObject compileScala(List<Path> files) throws IOException {
        long start = System.nanoTime();
        GradleRuns.Run run = newRun();
        Map<String, JsonObject> entries = new LinkedHashMap<>();
        List<ProjectSync.Plan> wanted = new ArrayList<>();
        for (JsonElement g : model.getAsJsonArray("gradleBuilds")) {
            JsonObject entry = g.getAsJsonObject();
            ProjectSync.Plan plan = plans.get(string(entry, "id"));
            if (plan != null && "synced".equals(string(entry, "status")) && Lsp.hasScala(entry)
                && (files.isEmpty() || ScalaBridge.holdsAny(entry, files))) {
                entries.put(plan.id(), entry);
                wanted.add(plan);
            }
        }
        JsonArray builds = new JsonArray();
        List<String> folders = new ArrayList<>();
        List<BuildProblems.Problem> problems = new ArrayList<>();
        try {
            // Several builds at once, as the bridge's first compile: a shared source folder
            // changes every build that compiles it.
            List<JsonObject> results = ProjectSync.parallel(wanted, ProjectSync.defaultParallelism(), plan -> {
                JsonObject entry = entries.get(plan.id());
                BuildProblems found = new BuildProblems();
                LineOutput output = new LineOutput(line -> {
                    found.accept(line);
                    GradleRuns.output(run, this, "stdout", line);
                });
                JsonObject b = new JsonObject();
                b.addProperty("build", plan.id());
                try {
                    ScalaBridge.Result r = ScalaBridge.compile(entry, plan.options(), output);
                    b.addProperty("millis", r.millis);
                    b.addProperty("compileMillis", r.compileMillis);
                    JsonArray tasks = new JsonArray();
                    r.tasks.forEach(tasks::add);
                    b.add("tasks", tasks);
                    b.addProperty("error", r.error);
                } catch (IOException | RuntimeException e) {
                    b.addProperty("error", SnoutEngine.describe(e));
                } finally {
                    output.close();
                }
                synchronized (problems) {
                    problems.addAll(found.finish());
                    folders.addAll(ScalaBridge.classFolders(entry));
                }
                return b;
            });
            results.forEach(builds::add);
            JsonObject p = new JsonObject();
            p.addProperty("runId", run.id);
            p.add("problems", BuildProblems.toJson(new ArrayList<>(new java.util.LinkedHashSet<>(problems))));
            notify("snout/buildProblems", p);
            if (!folders.isEmpty() && jdtls != null) {
                JsonArray args = new JsonArray();
                folders.forEach(args::add);
                long refresh = System.nanoTime();
                try {
                    executeCommand("snout.refreshClassFolders", args, 120);
                    SpringSupport s = spring;
                    if (s != null) {
                        s.scalaCompiled();
                    }
                    log.println("snout-engine: refreshed " + folders.size() + " Scala class folder(s) in jdtls in "
                        + (System.nanoTime() - refresh) / 1_000_000 + " ms");
                } catch (Exception e) {
                    log.println("snout-engine: could not refresh the Scala class folders in jdtls: " + SnoutEngine.describe(e));
                }
            }
        } finally {
            runs.remove(run.id);
        }
        JsonObject result = new JsonObject();
        result.addProperty("runId", run.id);
        result.add("builds", builds);
        result.addProperty("millis", (System.nanoTime() - start) / 1_000_000);
        boolean failed = builds.asList().stream().anyMatch(b -> b.getAsJsonObject().has("error") && !b.getAsJsonObject().get("error").isJsonNull());
        GradleRuns.finish(run, this, failed ? "failed" : "success", null, null);
        return result;
    }

    // ---- helpers for Launches

    JsonObject model() {
        return model;
    }

    PrintStream log() {
        return log;
    }

    Path runDir() {
        return runDir;
    }

    /** jdtls's workspace, which holds the JDT projects the importer creates (next to the run folder). */
    Path workspaceDir() {
        return runDir.resolveSibling("workspace");
    }

    ExecutorService pool() {
        return pool;
    }

    GradleRuns.Run register() {
        return newRun();
    }

    void unregister(GradleRuns.Run run) {
        runs.remove(run.id);
    }

    /** Runs a jdtls command ({@code workspace/executeCommand}) once jdtls has imported the projects. */
    JsonElement executeCommand(String command, JsonArray arguments, int timeoutSeconds) throws Exception {
        Jdtls j = jdtls;
        if (j == null) {
            throw new RequestError(-32603, "jdtls is not running");
        }
        try {
            j.ready().get(timeoutSeconds, TimeUnit.SECONDS);
            JsonObject params = new JsonObject();
            params.addProperty("command", command);
            params.add("arguments", arguments);
            return j.request("workspace/executeCommand", params).get(timeoutSeconds, TimeUnit.SECONDS);
        } catch (TimeoutException e) {
            throw new RequestError(-32603, command + " timed out after " + timeoutSeconds + " s");
        } catch (ExecutionException e) {
            throw new RequestError(-32603, command + " failed: " + e.getCause().getMessage());
        }
    }

    /** Asks jdtls an LSP request once it is ready. */
    JsonElement jdtlsRequest(String method, JsonElement params, int timeoutSeconds) throws Exception {
        Jdtls j = jdtls;
        if (j == null) {
            throw new RequestError(-32603, "jdtls is not running");
        }
        try {
            j.ready().get(timeoutSeconds, TimeUnit.SECONDS);
            return j.request(method, params).get(timeoutSeconds, TimeUnit.SECONDS);
        } catch (TimeoutException e) {
            throw new RequestError(-32603, method + " timed out after " + timeoutSeconds + " s");
        } catch (ExecutionException e) {
            throw new RequestError(-32603, method + " failed: " + e.getCause().getMessage());
        }
    }

    // ---- lookups

    ProjectSync.Plan plan(String buildId) throws RequestError {
        ProjectSync.Plan plan;
        if (buildId == null) {
            if (plans.size() != 1) {
                throw RequestError.invalid("name the build: one of " + plans.keySet());
            }
            plan = plans.values().iterator().next();
        } else {
            plan = plans.get(buildId);
            if (plan == null) {
                throw RequestError.invalid("no build " + buildId + "; the builds are " + plans.keySet());
            }
        }
        if (plan.needsJdk() != null) {
            throw new RequestError(-32603, plan.id() + ": Needs JDK " + plan.needsJdk() + ", which isn't installed");
        }
        return plan;
    }

    Modules.Ref module(String id) throws RequestError {
        if (id == null) {
            throw RequestError.invalid("name the module");
        }
        Modules.Ref ref = Modules.find(model, id);
        if (ref == null) {
            throw RequestError.invalid("no module " + id);
        }
        return ref;
    }

    private static Path path(JsonObject params) throws RequestError {
        String uri = string(params, "uri");
        String path = string(params, "path");
        if (uri != null) {
            return toPath(uri);
        }
        if (path != null) {
            return Path.of(path);
        }
        throw RequestError.invalid("give a uri or a path");
    }

    static Path toPath(String uriOrPath) {
        if (uriOrPath.startsWith("file:")) {
            return Path.of(URI.create(uriOrPath));
        }
        return Path.of(uriOrPath);
    }

    private static String required(JsonObject o, String key) throws RequestError {
        String value = string(o, key);
        if (value == null || value.isBlank()) {
            throw RequestError.invalid("missing " + key);
        }
        return value;
    }

    static String string(JsonObject o, String key) {
        if (o == null) {
            return null;
        }
        JsonElement e = o.get(key);
        return e == null || e.isJsonNull() ? null : e.getAsString();
    }

    static JsonArray array(JsonObject o, String key) {
        JsonElement e = o == null ? null : o.get(key);
        return e != null && e.isJsonArray() ? e.getAsJsonArray() : null;
    }

}
