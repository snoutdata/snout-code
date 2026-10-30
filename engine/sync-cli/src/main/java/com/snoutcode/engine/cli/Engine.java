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
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
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
    private final ExecutorService refresher = Executors.newSingleThreadExecutor(r -> {
        Thread t = new Thread(r, "snout-refresh");
        t.setDaemon(true);
        return t;
    });
    private final Set<Path> openFiles = ConcurrentHashMap.newKeySet();
    private volatile ScalaBridge.Work bridge;
    private volatile Set<String> imported = Set.of();
    private volatile Readiness readiness;
    private volatile Path modelFile;
    private volatile Path bridgeState;
    private volatile boolean verbose;
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
            case "snout/mavenGoals" -> mavenGoals(params);
            case "snout/runTask" -> runTask(params);
            case "snout/cancelTask" -> cancel(params);
            case "snout/runTests" -> runTests(params);
            case "snout/testsInFile" -> testsInFile(params);
            case "snout/runConfigurations" -> runConfigurations(params);
            case "snout/run" -> run(params);
            case "snout/scalaChanged" -> scalaChanged(params, notification);
            case "snout/readiness" -> readiness == null ? new JsonObject() : readiness.snapshot();
            case "snout/springBeans" -> spring("snout.spring.beans", params);
            case "snout/springEndpoints" -> spring("snout.spring.endpoints", params);
            case "snout/springProfiles" -> spring("snout.spring.profiles", params);
            case "snout/springSetProfiles" -> {
                SpringSupport s = spring;
                if (s == null) {
                    throw new RequestError(-32603, "Spring support is off");
                }
                yield s.setProfiles(params);
            }
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
        refresher.shutdownNow();
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
            if (!plan.maven() && (only == null || only.equals(plan.id()))) {
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
        if (plan.maven()) {
            MavenRuns.Invocation inv = maven(plan, string(params, "project"), params);
            for (String goal : task.trim().split("\\s+")) {
                inv.goals.add(goal);
            }
            return startMaven(inv, null, newRun());
        }
        String project = string(params, "project");
        if (!task.startsWith(":") && project != null) {
            task = (project.equals(":") ? "" : project) + ":" + task;
        }
        GradleRuns.Invocation inv = new GradleRuns.Invocation();
        inv.options = plan.options();
        inv.tasks.add(task);
        inv.args.addAll(GradleRuns.strings(array(params, "args")));
        inv.env.putAll(GradleRuns.env(params));
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
        if (plan(module.buildId()).maven()) {
            return mavenTests(module, params);
        }
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
        inv.env.putAll(GradleRuns.env(params));
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
            case "maven" -> {
                ProjectSync.Plan plan = plan(string(config, "build"));
                Modules.Ref module = string(config, "module") == null ? null : module(string(config, "module"));
                MavenRuns.Invocation inv = maven(plan, module == null ? null : module.gradlePath(), config);
                for (String goal : String.valueOf(string(config, "goals")).trim().split("\\s+")) {
                    inv.goals.add(goal);
                }
                if (dryRun) {
                    JsonObject result = new JsonObject();
                    JsonArray commands = new JsonArray();
                    for (List<String> c : MavenRuns.commands(inv, null)) {
                        JsonArray a = new JsonArray();
                        c.forEach(a::add);
                        commands.add(a);
                    }
                    result.add("commands", commands);
                    result.addProperty("javaHome", plan.options().javaHome == null ? null : plan.options().javaHome.getAbsolutePath());
                    return result;
                }
                if (debug) {
                    inv.debugKind = inv.goals.stream().anyMatch(g -> g.endsWith(":run") && g.contains("spring-boot")) ? "boot" : "tests";
                    return debugMaven(inv, null);
                }
                return startMaven(inv, null, newRun());
            }
            case "gradle" -> {
                GradleRuns.Invocation inv = new GradleRuns.Invocation();
                inv.options = plan(string(config, "build")).options();
                inv.tasks.add(string(config, "task"));
                inv.args.addAll(GradleRuns.strings(array(config, "args")));
                inv.env.putAll(GradleRuns.env(config));
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
                if (config.has("env")) {
                    tests.add("env", config.get("env"));
                }
                if (dryRun) {
                    GradleRuns.Invocation inv = new GradleRuns.Invocation();
                    inv.options = plan(string(config, "build")).options();
                    inv.tasks.add(string(config, "task"));
                    inv.env.putAll(GradleRuns.env(config));
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
        JsonObject env = new JsonObject();
        inv.env.forEach(env::addProperty);
        result.add("env", env);
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
        JsonObject attach = Launches.attachArguments(port, testModule == null ? null : Launches.jdtProject(model, testModule));
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

    // ---- Maven

    /** {@code {build?, refresh?}}: each Maven build's modules with their lifecycle, plugins and goals, and profiles. */
    private JsonElement mavenGoals(JsonObject params) throws Exception {
        String only = string(params, "build");
        boolean refresh = params.has("refresh") && params.get("refresh").getAsBoolean();
        JsonArray builds = new JsonArray();
        for (ProjectSync.Plan plan : plans.values()) {
            if (!plan.maven() || only != null && !only.equals(plan.id())) {
                continue;
            }
            if (refresh) {
                tasksCache.remove("maven:" + plan.id());
            }
            builds.add(tasksCache.computeIfAbsent("maven:" + plan.id(), k -> MavenGoals.of(plan, Modules.all(model))));
        }
        JsonObject result = new JsonObject();
        result.add("builds", builds);
        return result;
    }

    /**
     * A Maven invocation in a module (a module path, {@code :app}; null or {@code :} for the whole
     * build) with the options a request may carry: {@code args}, {@code env}, {@code profiles},
     * {@code offline}, {@code skipTests}.
     */
    private MavenRuns.Invocation maven(ProjectSync.Plan plan, String modulePath, JsonObject params) {
        MavenRuns.Invocation inv = new MavenRuns.Invocation();
        inv.plan = plan;
        List<Modules.Ref> reactor = new ArrayList<>();
        for (Modules.Ref m : Modules.all(model)) {
            if (m.buildId().equals(plan.id())) {
                reactor.add(m);
                if (modulePath != null && modulePath.equals(m.gradlePath())) {
                    inv.module = m;
                }
            }
        }
        inv.reactor = reactor;
        inv.args.addAll(GradleRuns.strings(array(params, "args")));
        inv.env.putAll(GradleRuns.env(params));
        inv.profiles.addAll(GradleRuns.strings(array(params, "profiles")));
        inv.offline = params.has("offline") && params.get("offline").getAsBoolean();
        inv.skipTests = params.has("skipTests") && params.get("skipTests").getAsBoolean();
        inv.runDir = runDir;
        return inv;
    }

    private JsonObject startMaven(MavenRuns.Invocation inv, String testModule, GradleRuns.Run run) {
        pool.submit(() -> {
            try {
                MavenRuns.execute(inv, run, this);
                if (testModule != null) {
                    lastFailedTests.put(testModule, new ArrayList<>(run.failedTests));
                }
            } finally {
                runs.remove(run.id);
            }
        });
        JsonObject result = new JsonObject();
        result.addProperty("runId", run.id);
        result.addProperty("tool", "maven");
        JsonArray tasks = new JsonArray();
        inv.goals.forEach(tasks::add);
        result.add("tasks", tasks);
        return result;
    }

    /** A module's tests through Maven (surefire), filtered, rerun or debugged as for Gradle. */
    private JsonElement mavenTests(Modules.Ref module, JsonObject params) throws Exception {
        MavenRuns.Invocation inv = maven(plan(module.buildId()), module.gradlePath(), params);
        inv.goals.add("test");
        inv.tests = true;
        boolean rerunFailed = params.has("rerunFailed") && params.get("rerunFailed").getAsBoolean();
        if (rerunFailed) {
            List<String[]> failed = lastFailedTests.getOrDefault(module.id(), List.of());
            if (failed.isEmpty()) {
                throw RequestError.invalid("no failed tests to rerun in " + module.id());
            }
            inv.testFilter = MavenRuns.rerunFilter(failed);
        } else {
            inv.testFilter = MavenRuns.testFilter(params.has("filter") && params.get("filter").isJsonObject()
                ? params.getAsJsonObject("filter") : new JsonObject());
        }
        if (params.has("debug") && params.get("debug").getAsBoolean()) {
            inv.debugKind = "tests";
            return debugMaven(inv, module.id());
        }
        return startMaven(inv, module.id(), newRun());
    }

    /** A Maven run whose test JVM (or {@code spring-boot:run}'s) waits for a debugger; as {@link #debugGradle}. */
    private JsonElement debugMaven(MavenRuns.Invocation inv, String testModule) throws Exception {
        int port = Launches.freePort();
        inv.debugPort = port;
        int dapPort = Launches.startDebugSession(this);
        // java-debug finds the sources of breakpoints through the module's JDT project.
        JsonObject attach = Launches.attachArguments(port, inv.module == null ? null : Launches.jdtProject(model, inv.module.id()));
        GradleRuns.Run run = newRun();
        AtomicBoolean ready = new AtomicBoolean();
        Runnable announce = () -> {
            if (ready.compareAndSet(false, true)) {
                JsonObject p = new JsonObject();
                p.addProperty("runId", run.id);
                p.addProperty("dapPort", dapPort);
                p.add("attach", attach);
                notify("snout/debugReady", p);
            }
        };
        inv.lineHook = line -> {
            if (GradleRuns.jdwpPort(line) > 0) {
                announce.run();
            }
        };
        // Surefire keeps its forked JVM's "Listening for transport" line to itself: the JVM
        // listens once the port it was given cannot be taken (connecting would end its wait).
        pool.submit(() -> {
            long deadline = System.nanoTime() + java.util.concurrent.TimeUnit.MINUTES.toNanos(30);
            while (!ready.get() && runs.containsKey(run.id) && System.nanoTime() < deadline) {
                try {
                    new java.net.ServerSocket(port, 1, java.net.InetAddress.getLoopbackAddress()).close();
                } catch (IOException e) {
                    announce.run();
                }
                try {
                    Thread.sleep(250);
                } catch (InterruptedException e) {
                    return;
                }
            }
        });
        JsonObject result = startMaven(inv, testModule, run);
        JsonObject debug = new JsonObject();
        debug.addProperty("dapPort", dapPort);
        debug.add("attach", attach);
        result.add("debug", debug);
        return result;
    }

    // ---- the Scala bridge, readiness and the model

    /** The bridge's plan for the model (the first {@code snout/scalaChanged} makes one when none was given). */
    void bridge(ScalaBridge.Work work) {
        this.bridge = work;
        if (work != null) {
            this.imported = work.folderSet();
        }
    }

    /** Where the model lives, rewritten when the bridge or a background sync changes it. */
    void modelFile(Path file) {
        this.modelFile = file;
    }

    void attach(Readiness readiness) {
        this.readiness = readiness;
    }

    /** The app opened or closed a document: the Scala of open files' builds compiles first. */
    void documentEvent(String method, JsonObject message) {
        JsonObject params = message.has("params") && message.get("params").isJsonObject() ? message.getAsJsonObject("params") : null;
        String uri = params == null ? null : string(object(params, "textDocument"), "uri");
        Path file = uri == null ? null : Readiness.path(uri);
        if (file == null) {
            return;
        }
        if ("textDocument/didOpen".equals(method)) {
            openFiles.add(file);
        } else {
            openFiles.remove(file);
        }
    }

    /** jdtls imported the projects (its {@code ServiceReady}). */
    void jdtlsReady() {
        Readiness r = readiness;
        if (r != null) {
            r.jdtlsReady();
        }
    }

    /** jdtls's diagnostics for a file, or null while they are held back (see {@link Readiness}). */
    JsonObject diagnostics(JsonObject message) {
        Readiness r = readiness;
        return r == null ? message : r.diagnostics(message);
    }

    /** The builds of the files open in the editor, whose Scala compiles first. */
    private List<String> preferredBuilds() {
        List<String> out = new ArrayList<>();
        Readiness r = readiness;
        if (r == null) {
            return out;
        }
        for (String module : r.modulesOf(openFiles)) {
            for (JsonElement m : array(model, "modules")) {
                JsonObject o = m.getAsJsonObject();
                if (module.equals(string(o, "id")) && string(o, "build") != null && !out.contains(string(o, "build"))) {
                    out.add(string(o, "build"));
                }
            }
        }
        return out;
    }

    /**
     * The bridge's first compile, in the background while jdtls starts: the units and builds
     * whose inputs changed since their last good compile, open files' builds first. Each one
     * done is refreshed in jdtls and its modules made ready.
     */
    void startScala(int parallel) {
        scalaQueue.submit(() -> {
            try {
                compileScala(ScalaBridge.Selection.all(), parallel, null);
            } catch (Exception | LinkageError e) {
                log.println("snout-engine: the Scala bridge failed: " + SnoutEngine.describe(e));
            }
        });
    }

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
                ScalaBridge.Work work = work();
                Set<String> units = new LinkedHashSet<>();
                Set<String> builds = new LinkedHashSet<>();
                if (files.isEmpty()) {
                    work.units.forEach(u -> units.add(u.key));
                    builds.addAll(work.builds());
                } else {
                    for (ScalaBridge.Unit u : ScalaBridge.unitsHolding(work, files)) {
                        units.add(u.key);
                        u.users.forEach(c -> builds.add(c.buildId));
                    }
                    builds.addAll(ScalaBridge.buildsHolding(work, files));
                }
                GradleRuns.Run run = newRun();
                done.complete(compileScala(new ScalaBridge.Selection(builds, builds, units), ProjectSync.defaultParallelism(), run));
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

    private ScalaBridge.Work work() throws IOException {
        if (bridge == null) {
            ScalaBridge.Work work = ScalaBridge.plan(model, plans.values(), null);
            ScalaBridge.annotate(work);
            bridge(work);
        }
        return bridge;
    }

    /**
     * Runs the bridge on a selection. With a {@code run} (a {@code snout/scalaChanged}), Gradle's
     * output goes to the app as that run's, with its problems; without, it is the start's
     * compile, which only logs. Waits until jdtls has read the new classes.
     */
    private JsonObject compileScala(ScalaBridge.Selection selection, int parallel, GradleRuns.Run run) throws Exception {
        long start = System.nanoTime();
        ScalaBridge.Work work = work();
        Readiness r = readiness;
        Map<String, BuildProblems> problems = new ConcurrentHashMap<>();
        Map<String, LineOutput> outputs = new ConcurrentHashMap<>();
        List<CompletableFuture<Void>> refreshed = new ArrayList<>();
        JsonArray builds = new JsonArray();
        List<String> selected = new ArrayList<>();
        for (String b : work.builds()) {
            if (selection.builds() == null || selection.builds().contains(b)) {
                selected.add(b);
            }
        }
        List<ScalaBridge.Unit> units = work.units.stream()
            .filter(u -> u.active && (selection.forcedUnits().contains(u.key) || u.users.stream().anyMatch(c -> selected.contains(c.buildId))))
            .toList();
        if (selected.isEmpty() && units.isEmpty()) {
            JsonObject result = new JsonObject();
            result.addProperty("runId", run == null ? null : run.id);
            result.add("builds", builds);
            result.addProperty("millis", 0);
            if (run != null) {
                runs.remove(run.id);
                GradleRuns.finish(run, this, "success", null, null);
            }
            return result;
        }
        if (run == null) {
            log.println("snout-engine: compiling Scala in " + selected.size() + " build(s)"
                + (units.isEmpty() ? "" : ", " + units.size() + " shared folder unit(s) once") + ", in the background");
        }
        if (r != null) {
            for (ScalaBridge.Unit u : units) {
                r.compiling(List.of(u.module(model)), work.lastMillis("units", u.key));
            }
            for (String b : selected) {
                long expected = work.lastMillis("builds", b);
                for (ScalaBridge.Unit u : work.unitsOf(b)) {
                    if (units.contains(u) && work.lastMillis("units", u.key) > 0 && expected > 0) {
                        expected += work.lastMillis("units", u.key);
                    }
                }
                r.compiling(work.modulesOf(b), expected);
            }
        }
        ScalaBridge.Listener listener = new ScalaBridge.Listener() {
            @Override
            public void unitFinished(ScalaBridge.Unit u, ScalaBridge.Result result) {
                String module = u.module(model);
                log.println("snout-engine: " + (result.upToDate ? "the shared Scala of " + module + " is up to date"
                    : "compiled the shared Scala of " + module + " once for " + u.users.size() + " compile(s) in "
                    + result.compileMillis + " ms") + (result.error == null ? "" : "; " + result.error));
                List<String> folders = new ArrayList<>();
                u.folders.forEach(f -> folders.add(f.classesDir().toString()));
                refreshed.add(bridged(folders, List.of(module), !result.upToDate));
            }

            @Override
            public void buildFinished(String b, ScalaBridge.Result result) {
                log.println("snout-engine: " + (result.upToDate ? "Scala in " + b + " is up to date"
                    : "compiled Scala in " + b + " (" + String.join(" ", result.tasks) + ") in " + result.compileMillis + " ms")
                    + (result.error == null ? "" : "; the compile failed: " + result.error));
                refreshed.add(bridged(work.classFolders(b), work.modulesOf(b), !result.upToDate));
            }
        };
        Map<String, ScalaBridge.Result> results;
        try {
            results = ScalaBridge.run(work, selection, parallel, this::preferredBuilds, listener, b -> {
                if (run == null) {
                    return verbose ? log : null;
                }
                BuildProblems found = problems.computeIfAbsent(b, k -> new BuildProblems());
                return outputs.computeIfAbsent(b, k -> new LineOutput(line -> {
                    found.accept(line);
                    GradleRuns.output(run, this, "stdout", line);
                }));
            });
        } finally {
            outputs.values().forEach(LineOutput::close);
        }
        for (Map.Entry<String, ScalaBridge.Result> e : results.entrySet()) {
            JsonObject b = new JsonObject();
            b.addProperty("build", e.getKey());
            b.addProperty("millis", e.getValue().millis);
            b.addProperty("compileMillis", e.getValue().compileMillis);
            JsonArray tasks = new JsonArray();
            e.getValue().tasks.forEach(tasks::add);
            b.add("tasks", tasks);
            b.addProperty("upToDate", e.getValue().upToDate);
            b.addProperty("error", e.getValue().error);
            builds.add(b);
        }
        writeModel();
        // A unit that fell back, or a new "other" folder: jdtls must import the class folders again.
        Set<String> now = work.folderSet();
        if (!now.equals(imported)) {
            imported = now;
            refreshed.add(reimport("the Scala class folders changed"));
        }
        for (CompletableFuture<Void> f : refreshed) {
            try {
                f.get(10, TimeUnit.MINUTES);
            } catch (ExecutionException | TimeoutException e) {
                log.println("snout-engine: jdtls did not take the new Scala classes: " + SnoutEngine.describe(e));
            }
        }
        long millis = (System.nanoTime() - start) / 1_000_000;
        long failed = results.values().stream().filter(x -> x.error != null).count();
        if (run == null) {
            log.println("snout-engine: compiled Scala in " + results.size() + " build(s) in " + millis + " ms"
                + (failed == 0 ? "" : "; " + failed + " build(s) failed to compile"));
        }
        JsonObject result = new JsonObject();
        if (run != null) {
            List<BuildProblems.Problem> all = new ArrayList<>();
            problems.values().forEach(bp -> all.addAll(bp.finish()));
            JsonObject p = new JsonObject();
            p.addProperty("runId", run.id);
            p.add("problems", BuildProblems.toJson(new ArrayList<>(new LinkedHashSet<>(all))));
            notify("snout/buildProblems", p);
            runs.remove(run.id);
            GradleRuns.finish(run, this, failed > 0 ? "failed" : "success", null, null);
            result.addProperty("runId", run.id);
        }
        result.add("builds", builds);
        result.addProperty("millis", millis);
        return result;
    }

    /**
     * Once jdtls is up, makes it read class folders the bridge filled (when {@code changed}) and
     * build, then marks the modules ready. Requests queue on one thread and the ones waiting are
     * taken together, so a burst of compiles costs one refresh and one build.
     */
    private CompletableFuture<Void> bridged(List<String> folders, List<String> modules, boolean changed) {
        Readiness r = readiness;
        if (r != null) {
            r.compiled(modules);
        }
        Pending p = new Pending(folders, modules, changed, null);
        pending.add(p);
        refresher.submit(this::drain);
        return p.done;
    }

    private CompletableFuture<Void> reimport(String why) {
        Pending p = new Pending(List.of(), List.of(), true, why);
        pending.add(p);
        refresher.submit(this::drain);
        return p.done;
    }

    private record Pending(List<String> folders, List<String> modules, boolean changed, String reimport,
                           CompletableFuture<Void> done) {
        Pending(List<String> folders, List<String> modules, boolean changed, String reimport) {
            this(folders, modules, changed, reimport, new CompletableFuture<>());
        }
    }

    private final java.util.concurrent.ConcurrentLinkedQueue<Pending> pending = new java.util.concurrent.ConcurrentLinkedQueue<>();

    private void drain() {
        List<Pending> batch = new ArrayList<>();
        for (Pending p; (p = pending.poll()) != null; ) {
            batch.add(p);
        }
        if (batch.isEmpty()) {
            return;
        }
        try {
            Jdtls j = jdtls;
            if (j != null) {
                j.ready().get();
                long start = System.nanoTime();
                String why = batch.stream().map(Pending::reimport).filter(java.util.Objects::nonNull).findFirst().orElse(null);
                if (why != null) {
                    executeCommand("snout.reimport", new JsonArray(), 600);
                    log.println("snout-engine: imported the projects into jdtls again (" + why + ")");
                }
                JsonArray folders = new JsonArray();
                batch.stream().filter(Pending::changed).forEach(p -> p.folders().forEach(folders::add));
                if (!folders.isEmpty()) {
                    executeCommand("snout.refreshClassFolders", folders, 120);
                    SpringSupport s = spring;
                    if (s != null) {
                        s.scalaCompiled();
                    }
                }
                if (why != null || !folders.isEmpty()) {
                    j.request("java/buildWorkspace", new com.google.gson.JsonPrimitive(false)).get(30, TimeUnit.MINUTES);
                    log.println("snout-engine: jdtls read " + folders.size() + " Scala class folder(s) and built in "
                        + (System.nanoTime() - start) / 1_000_000 + " ms");
                }
            }
        } catch (Exception e) {
            log.println("snout-engine: could not refresh the Scala class folders in jdtls: " + SnoutEngine.describe(e));
        } finally {
            Readiness r = readiness;
            List<String> modules = new ArrayList<>();
            batch.forEach(p -> modules.addAll(p.modules()));
            if (r != null) {
                r.ready(modules);
            }
            batch.forEach(p -> p.done().complete(null));
        }
    }

    private void writeModel() {
        Path file = modelFile;
        if (file == null) {
            return;
        }
        synchronized (model) {
            try {
                Lsp.writeAtomically(file, new com.google.gson.GsonBuilder().setPrettyPrinting().disableHtmlEscaping().create().toJson(model));
            } catch (IOException e) {
                log.println("snout-engine: could not write the model: " + SnoutEngine.describe(e));
            }
        }
    }

    // ---- syncing again in the background

    /**
     * After a start from the cached model: syncs the builds whose sync may come out differently
     * (see {@link ModelCache#uncertain}) once the Scala bridge is done, and when their model
     * changed, takes it: the model file and its inputs, the bridge, jdtls's projects.
     */
    void resyncLater(Set<String> builds, ProjectSync.Options options, Runnable recordInputs) {
        scalaQueue.submit(() -> {
            try {
                List<ProjectSync.Plan> wanted = plans.values().stream().filter(p -> builds.contains(p.id())).toList();
                if (wanted.isEmpty()) {
                    return;
                }
                log.println("snout-engine: syncing " + wanted.size() + " build(s) again in the background: " + String.join(", ", builds));
                List<JsonObject> entries = ProjectSync.lanes(wanted, options.parallel, plan -> ProjectSync.syncOne(plan, options.log));
                List<String> changed = new ArrayList<>();
                synchronized (model) {
                    JsonArray all = model.getAsJsonArray("gradleBuilds");
                    for (JsonObject fresh : entries) {
                        for (int i = 0; i < all.size(); i++) {
                            JsonObject old = all.get(i).getAsJsonObject();
                            if (!string(old, "id").equals(string(fresh, "id"))) {
                                continue;
                            }
                            if ("failed".equals(string(fresh, "status")) && !"failed".equals(string(old, "status"))) {
                                break; // keep the last good model, as a start does
                            }
                            if (!ScalaBridge.canonicalHash(old).equals(ScalaBridge.canonicalHash(fresh))) {
                                all.set(i, fresh);
                                changed.add(string(fresh, "id"));
                            }
                        }
                    }
                    if (!changed.isEmpty()) {
                        ProjectModel.assemble(model);
                    }
                }
                recordInputs.run();
                if (changed.isEmpty()) {
                    log.println("snout-engine: the background sync found nothing new");
                    return;
                }
                log.println("snout-engine: the background sync changed " + String.join(", ", changed));
                ScalaBridge.Work work = ScalaBridge.plan(model, plans.values(), bridge == null ? null : bridgeState);
                ScalaBridge.annotate(work);
                bridge = work;
                Readiness r = readiness;
                if (r != null) {
                    r.model(model);
                }
                writeModel();
                imported = work.folderSet();
                reimport("the background sync changed " + String.join(", ", changed)).get(10, TimeUnit.MINUTES);
                compileScala(new ScalaBridge.Selection(new LinkedHashSet<>(changed), Set.of(), Set.of()), ProjectSync.defaultParallelism(), null);
                notify("snout/modelChanged", new JsonObject());
            } catch (Exception | LinkageError e) {
                log.println("snout-engine: the background sync failed: " + SnoutEngine.describe(e));
            }
        });
    }

    /** Where the bridge keeps its state, for a plan made after a background sync. */
    void bridgeState(Path file) {
        this.bridgeState = file;
    }

    void verbose(boolean verbose) {
        this.verbose = verbose;
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

    static JsonObject object(JsonObject o, String key) {
        return o == null || !o.has(key) || !o.get(key).isJsonObject() ? null : o.getAsJsonObject(key);
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
