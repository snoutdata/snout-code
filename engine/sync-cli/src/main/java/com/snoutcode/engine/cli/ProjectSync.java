package com.snoutcode.engine.cli;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.snoutcode.engine.sync.ProjectModelBuilder;

import java.io.File;
import java.io.IOException;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.function.Function;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * Syncs a folder that holds one or many Gradle builds into one project model: finds the builds
 * ({@link Discovery}), syncs each on a JDK its Gradle version runs on ({@link JavaHomes}), several
 * at a time, and assembles the result ({@link ProjectModel}). A build that fails does not fail
 * the others: its entry carries the error, the Gradle version and a short reason.
 */
public final class ProjectSync {
    /** The model format: version 2 holds many Gradle builds. */
    public static final int MODEL_VERSION = 2;

    /** Options of a project sync, on top of the per-build {@link Sync.Options}. */
    public static final class Options {
        /** The folder; also the defaults for every build (Gradle user home, version, offline...). */
        public final Sync.Options sync = new Sync.Options();
        /** Extra JDKs to choose from, besides the ones found on the machine. */
        public final List<File> jdks = new ArrayList<>();
        /** Where SnoutCode keeps JDKs it downloaded, searched too; may be null. */
        public Path cacheDir;
        /** How many builds sync at once: at most one per Gradle version and JDK (see {@link #lanes}). */
        public int parallel = defaultParallelism();
        /** Progress lines, or null. */
        public PrintStream log;
        /** Maven profiles to activate ({@code a}) or deactivate ({@code !a}) in every Maven build. */
        public final List<String> mavenProfiles = new ArrayList<>();
    }

    /**
     * One build to sync; {@code needsJdk} is the Java major it needs when no installed JDK runs its
     * Gradle (or compiles its Maven build). A Maven build ({@code maven}) has its Maven version
     * in {@code gradleVersion}, as {@code maven-<version>}.
     */
    public record Plan(String id, File rootDir, String gradleVersion, JavaHomes.Jdk jdk, Sync.Options options, Integer needsJdk,
                       boolean maven, MavenSync.Options mavenOptions) {
        public Plan(String id, File rootDir, String gradleVersion, JavaHomes.Jdk jdk, Sync.Options options, Integer needsJdk) {
            this(id, rootDir, gradleVersion, jdk, options, needsJdk, false, null);
        }
    }

    private ProjectSync() {
    }

    static int defaultParallelism() {
        return Math.max(1, Math.min(4, Runtime.getRuntime().availableProcessors() / 2));
    }

    /** Discovers, syncs and assembles. */
    public static JsonObject run(Options options) throws IOException {
        return sync(options, plan(options));
    }

    /** Syncs the planned builds, several at once, and assembles the project model. */
    public static JsonObject sync(Options options, List<Plan> plans) throws IOException {
        long start = System.nanoTime();
        File folder = options.sync.projectDir.getCanonicalFile();
        List<JsonObject> results = lanes(plans, options.parallel, plan -> syncOne(plan, options.log));
        JsonObject model = new JsonObject();
        model.addProperty("modelVersion", MODEL_VERSION);
        model.addProperty("pluginModelVersion", ProjectModelBuilder.MODEL_VERSION);
        model.addProperty("rootDir", folder.getAbsolutePath());
        model.addProperty("name", folder.getName());
        JsonArray builds = new JsonArray();
        results.forEach(builds::add);
        model.add("gradleBuilds", builds);
        ProjectModel.assemble(model);
        model.addProperty("syncMillis", (System.nanoTime() - start) / 1_000_000);
        return model;
    }

    /**
     * How long a start waits for the last builds once most are done (see {@link #syncEarly}): no
     * build finished for this long, and those still syncing are at most a third of them.
     */
    static final long LATE_QUIET_MILLIS = 2000;

    /** A sync that went on without some builds: theirs come when {@code late} completes. */
    public record Early(JsonObject model, Map<Plan, CompletableFuture<JsonObject>> late) {
    }

    /**
     * Syncs the planned builds as {@link #sync} does, but does not wait for the last few: once
     * at most a third of the builds still sync and none finished for {@link #LATE_QUIET_MILLIS}
     * (a build that is slow to configure, or slow to fail), the model is assembled without them.
     * Each of those is in the model as a placeholder, {@code status: "syncing"} and
     * {@code late: true}, and its sync goes on: its entry completes its future in {@code late}.
     */
    public static Early syncEarly(Options options, List<Plan> plans, long quietMillis) throws IOException {
        long start = System.nanoTime();
        File folder = options.sync.projectDir.getCanonicalFile();
        Map<Plan, CompletableFuture<JsonObject>> futures = new LinkedHashMap<>();
        plans.forEach(p -> futures.put(p, new CompletableFuture<>()));
        Object changed = new Object();
        long[] lastDone = {System.nanoTime()};
        Thread lanes = new Thread(() -> {
            try {
                lanes(plans, options.parallel, plan -> {
                    JsonObject entry = syncOne(plan, options.log);
                    synchronized (changed) {
                        futures.get(plan).complete(entry);
                        lastDone[0] = System.nanoTime();
                        changed.notifyAll();
                    }
                    return entry;
                });
            } catch (IOException | RuntimeException e) {
                futures.values().forEach(f -> f.completeExceptionally(e));
            } finally {
                synchronized (changed) {
                    changed.notifyAll();
                }
            }
        }, "snout-sync-lanes");
        lanes.setDaemon(true);
        lanes.start();
        synchronized (changed) {
            while (true) {
                long pending = futures.values().stream().filter(f -> !f.isDone()).count();
                if (pending == 0) {
                    break;
                }
                long quiet = (System.nanoTime() - lastDone[0]) / 1_000_000;
                if (goOn(pending, plans.size(), quiet, quietMillis)) {
                    break;
                }
                try {
                    changed.wait(Math.max(10, quietMillis - quiet));
                } catch (InterruptedException e) {
                    Thread.currentThread().interrupt();
                    throw new IOException("interrupted", e);
                }
            }
        }
        JsonArray builds = new JsonArray();
        Map<Plan, CompletableFuture<JsonObject>> late = new LinkedHashMap<>();
        for (Map.Entry<Plan, CompletableFuture<JsonObject>> e : futures.entrySet()) {
            CompletableFuture<JsonObject> f = e.getValue();
            if (f.isDone() && !f.isCompletedExceptionally()) {
                builds.add(f.join());
            } else if (f.isDone()) {
                throw new IOException(f.handle((v, x) -> x).join());
            } else {
                builds.add(placeholder(e.getKey()));
                late.put(e.getKey(), f);
            }
        }
        JsonObject model = new JsonObject();
        model.addProperty("modelVersion", MODEL_VERSION);
        model.addProperty("pluginModelVersion", ProjectModelBuilder.MODEL_VERSION);
        model.addProperty("rootDir", folder.getAbsolutePath());
        model.addProperty("name", folder.getName());
        model.add("gradleBuilds", builds);
        ProjectModel.assemble(model);
        model.addProperty("syncMillis", (System.nanoTime() - start) / 1_000_000);
        return new Early(model, late);
    }

    /**
     * Whether a start goes on without the {@code pending} builds of {@code total}: some are done,
     * at most a third still sync, and none finished for {@code quietMillis}.
     */
    static boolean goOn(long pending, long total, long quiet, long quietMillis) {
        return pending > 0 && pending < total && pending * 3 <= total && quiet >= quietMillis;
    }

    /** A build still syncing, in a model that went on without it. */
    static JsonObject placeholder(Plan plan) {
        JsonObject out = new JsonObject();
        out.addProperty("id", plan.id());
        out.addProperty("rootDir", plan.rootDir().getAbsolutePath());
        if (plan.maven()) {
            out.addProperty("tool", "maven");
            out.addProperty("mavenVersion", plan.gradleVersion() == null ? null : plan.gradleVersion().replaceFirst("^maven-", ""));
        } else {
            out.addProperty("gradleVersion", plan.gradleVersion());
        }
        out.addProperty("javaVersion", plan.jdk() == null ? null : plan.jdk().version());
        out.add("builds", new JsonArray());
        out.addProperty("status", "syncing");
        out.addProperty("late", true);
        out.addProperty("reason", "Still syncing");
        return out;
    }

    /** The builds of the folder, each with its id, Gradle version, JDK and own options. */
    public static List<Plan> plan(Options options) throws IOException {
        File folder = options.sync.projectDir.getCanonicalFile();
        List<JavaHomes.Jdk> installed = JavaHomes.discover(options.jdks, options.cacheDir);
        JavaHomes.Jdk preferred = options.sync.javaHome == null ? null : JavaHomes.read(options.sync.javaHome);
        List<Plan> plans = new ArrayList<>();
        for (File root : Discovery.builds(folder)) {
            String id = buildId(folder, root);
            if (Discovery.isMaven(root)) {
                plans.add(mavenPlan(id, root, options, preferred, installed));
                continue;
            }
            String version = options.sync.gradleVersion != null ? options.sync.gradleVersion : JavaHomes.wrapperVersion(root);
            Integer level = JavaHomes.scriptLevel(root);
            JavaHomes.Jdk jdk = JavaHomes.choose(version, level, preferred, installed);
            Sync.Options o = copy(options.sync);
            o.projectDir = root;
            Integer needs = JavaHomes.needs(version, level, jdk);
            // Never a JDK the build's Gradle cannot run on: with none installed, it needs one.
            o.javaHome = jdk != null ? jdk.home() : needs != null ? null : options.sync.javaHome;
            if (options.sync.projectCacheDir != null) {
                o.projectCacheDir = new File(options.sync.projectCacheDir, key(root));
            }
            plans.add(new Plan(id, root, version, jdk, o, needs));
        }
        return plans;
    }

    /**
     * A Maven build's plan: the JDK it compiles with is one of its Java level (the newest
     * {@code release}, or {@code source}, of its POMs), else the preferred JDK when it is at least
     * that, else the nearest newer one installed; with none, it needs the LTS at or above it.
     */
    static Plan mavenPlan(String id, File root, Options options, JavaHomes.Jdk preferred, List<JavaHomes.Jdk> installed) {
        int level = MavenLevels.level(root);
        JavaHomes.Jdk jdk = MavenLevels.choose(level, preferred, installed);
        Sync.Options o = copy(options.sync);
        o.projectDir = root;
        o.javaHome = jdk == null ? null : jdk.home();
        MavenSync.Options m = new MavenSync.Options();
        m.rootDir = root;
        m.javaHome = o.javaHome;
        m.jdks = installed;
        m.offline = options.sync.offline;
        m.sources = options.sync.sources;
        m.profiles = List.copyOf(options.mavenProfiles);
        Integer needs = jdk == null ? MavenLevels.lts(level) : null;
        return new Plan(id, root, "maven-" + MavenHome.wrapperVersion(root), jdk, o, needs, true, m);
    }

    /** One build's entry: its sync model, or what went wrong. Never throws. */
    static JsonObject syncOne(Plan plan, PrintStream log) {
        long start = System.nanoTime();
        JsonObject entry;
        if (plan.maven()) {
            try {
                if (plan.needsJdk() != null) {
                    throw new IOException("Needs JDK " + plan.needsJdk() + ", which isn't installed");
                }
                entry = MavenSync.run(plan.mavenOptions());
                entry.addProperty("status", "synced");
            } catch (Exception | LinkageError e) {
                entry = new JsonObject();
                entry.addProperty("tool", "maven");
                entry.addProperty("rootDir", plan.rootDir().getAbsolutePath());
                entry.addProperty("mavenVersion", MavenHome.wrapperVersion(plan.rootDir()));
                entry.addProperty("gradleJavaHome", plan.options().javaHome == null ? null : plan.options().javaHome.getAbsolutePath());
                entry.add("builds", new JsonArray());
                String error = SnoutEngine.describe(e);
                entry.addProperty("status", "failed");
                entry.addProperty("error", error);
                entry.addProperty("reason", MavenLevels.reason(error));
                if (plan.needsJdk() != null) {
                    entry.addProperty("needsJdk", plan.needsJdk());
                }
            }
            JsonObject out = new JsonObject();
            out.addProperty("id", plan.id());
            out.addProperty("javaVersion", plan.jdk() == null ? null : plan.jdk().version());
            entry.entrySet().forEach(e -> out.add(e.getKey(), e.getValue()));
            out.addProperty("syncMillis", (System.nanoTime() - start) / 1_000_000);
            if (log != null) {
                String what = "synced".equals(out.get("status").getAsString())
                    ? "synced " + plan.id() : "could not sync " + plan.id() + ": " + out.get("reason").getAsString();
                log.println("snout-engine: " + what + " (Maven, Java " + (plan.jdk() == null ? "?" : plan.jdk().feature()) + ") in "
                    + out.get("syncMillis").getAsLong() + " ms");
            }
            return out;
        }
        try {
            if (plan.needsJdk() != null) {
                // No installed JDK runs this Gradle version: say which one to install, rather than
                // fail on a JDK that cannot work.
                throw new MissingJdk(plan.needsJdk(), plan.gradleVersion());
            }
            entry = Sync.run(plan.options());
            entry.remove("modelVersion");
            entry.addProperty("status", "synced");
        } catch (Exception | LinkageError e) {
            entry = new JsonObject();
            entry.addProperty("rootDir", plan.rootDir().getAbsolutePath());
            entry.addProperty("gradleVersion", plan.gradleVersion());
            entry.addProperty("gradleJavaHome", plan.options().javaHome == null ? null : plan.options().javaHome.getAbsolutePath());
            entry.add("builds", new JsonArray());
            String error = SnoutEngine.describe(e);
            entry.addProperty("status", "failed");
            entry.addProperty("error", error);
            if (e instanceof MissingJdk missing) {
                entry.addProperty("error", missing.getMessage() + ": Gradle " + missing.gradleVersion + " runs on Java "
                    + JavaHomes.minJava(missing.gradleVersion) + " to " + JavaHomes.maxJava(missing.gradleVersion)
                    + ", and no JDK in that range is installed.");
                entry.addProperty("reason", missing.getMessage());
                entry.addProperty("needsJdk", missing.major);
            } else {
                entry.addProperty("reason", reason(error, plan.gradleVersion(), plan.jdk()));
                Integer major = jdkMismatch(error, plan.gradleVersion(), plan.jdk());
                if (major != null) {
                    // The JDK chosen does not run this build after all: say which one would.
                    entry.addProperty("needsJdk", major);
                }
            }
        }
        JsonObject out = new JsonObject();
        out.addProperty("id", plan.id());
        out.addProperty("javaVersion", plan.jdk() == null ? null : plan.jdk().version());
        entry.entrySet().forEach(e -> out.add(e.getKey(), e.getValue()));
        out.addProperty("syncMillis", (System.nanoTime() - start) / 1_000_000);
        if (log != null) {
            String what = "synced".equals(out.get("status").getAsString())
                ? "synced " + plan.id() : "could not sync " + plan.id() + ": " + out.get("reason").getAsString();
            log.println("snout-engine: " + what + " (Gradle " + (plan.gradleVersion() == null ? "?" : plan.gradleVersion())
                + ", Java " + (plan.jdk() == null ? "?" : plan.jdk().feature()) + ") in " + out.get("syncMillis").getAsLong() + " ms");
        }
        return out;
    }

    /** A build whose Gradle no installed JDK runs. */
    static final class MissingJdk extends Exception {
        private static final long serialVersionUID = 1L;
        final int major;
        final String gradleVersion;

        MissingJdk(int major, String gradleVersion) {
            super("Needs JDK " + major + ", which isn't installed");
            this.major = major;
            this.gradleVersion = gradleVersion;
        }
    }

    /**
     * Runs a task per plan in lanes, one lane per Gradle version and JDK, at most {@code limit}
     * lanes at once, the plans of a lane one after another; returns the results in plan order.
     *
     * <p>Builds of one lane share a daemon, so after the first they start warm. Running two of
     * them at once would need a second daemon, and the daemons would then take turns at the
     * locks of the shared dependency cache: on 12 builds of one Gradle version, four at a time
     * took three times as long as one at a time. Lanes of different versions still overlap their
     * daemons' start.
     */
    public static <T> List<T> lanes(List<Plan> plans, int limit, Function<Plan, T> task) throws IOException {
        Map<String, List<Plan>> lanes = new LinkedHashMap<>();
        for (Plan plan : plans) {
            String key = plan.gradleVersion() + "|" + plan.options().javaHome;
            lanes.computeIfAbsent(key, k -> new ArrayList<>()).add(plan);
        }
        Map<Plan, T> results = new ConcurrentHashMap<>();
        List<Plan> leaders = new ArrayList<>();
        Map<Plan, List<Plan>> laneOf = new HashMap<>();
        for (List<Plan> lane : lanes.values()) {
            leaders.add(lane.get(0));
            laneOf.put(lane.get(0), lane);
        }
        parallel(leaders, limit, leader -> {
            for (Plan plan : laneOf.get(leader)) {
                results.put(plan, task.apply(plan));
            }
            return null;
        });
        List<T> out = new ArrayList<>();
        plans.forEach(p -> out.add(results.get(p)));
        return out;
    }

    /** Runs a task per plan, at most {@code limit} at once, and returns the results in plan order. */
    public static <T> List<T> parallel(List<Plan> plans, int limit, Function<Plan, T> task) throws IOException {
        List<T> out = new ArrayList<>();
        if (plans.isEmpty()) {
            return out;
        }
        ExecutorService pool = Executors.newFixedThreadPool(Math.max(1, Math.min(limit, plans.size())), r -> {
            Thread t = new Thread(r, "snout-sync");
            t.setDaemon(true);
            return t;
        });
        try {
            List<Future<T>> futures = new ArrayList<>();
            for (Plan plan : plans) {
                futures.add(pool.submit(() -> task.apply(plan)));
            }
            for (Future<T> f : futures) {
                out.add(f.get());
            }
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            throw new IOException("interrupted", e);
        } catch (ExecutionException e) {
            throw new IOException(e.getCause());
        } finally {
            pool.shutdownNow();
        }
        return out;
    }

    /**
     * A build's id: its path relative to the folder, with {@code /} separators, or the folder's
     * name when the build is the folder itself or lies above it.
     */
    static String buildId(File folder, File root) {
        Path f = folder.toPath();
        Path r = root.toPath();
        if (r.startsWith(f) && !r.equals(f)) {
            return f.relativize(r).toString().replace(File.separatorChar, '/');
        }
        return r.getFileName() == null ? r.toString() : r.getFileName().toString();
    }

    /** A cache folder name for a build: readable, and unique by a hash of its path. */
    static String key(File root) {
        String hash = Sync.sha256(root.getAbsolutePath().getBytes(StandardCharsets.UTF_8)).substring(0, 12);
        return root.getName().replaceAll("[^A-Za-z0-9._-]", "_") + "-" + hash;
    }

    private static Sync.Options copy(Sync.Options from) {
        Sync.Options o = new Sync.Options();
        o.projectDir = from.projectDir;
        o.javaHome = from.javaHome;
        o.gradleUserHome = from.gradleUserHome;
        o.projectCacheDir = from.projectCacheDir;
        o.gradleVersion = from.gradleVersion;
        o.sources = from.sources;
        o.offline = from.offline;
        o.gradleOutput = from.gradleOutput;
        o.buildRoot = from.buildRoot;
        o.bridgeRoot = from.bridgeRoot;
        o.daemonRegistry = from.daemonRegistry;
        return o;
    }

    // ---- why a build failed, in a few words

    /** Errors of a Gradle (its Groovy) that runs on a Java newer than it supports. */
    private static final Pattern GROOVY_ON_NEW_JAVA = Pattern.compile(
        "GroovyBugError|Unsupported class file major version|NoClassDefFoundError: Could not initialize class org\\.codehaus\\.groovy"
            + "|Could not initialize class org\\.codehaus\\.groovy|BUG! exception in phase");

    /**
     * When a sync failed because its Gradle cannot run on the JDK it got (a class it cannot read,
     * Groovy failing inside Gradle), the LTS Java that Gradle runs on, for {@code needsJdk}; else null.
     */
    static Integer jdkMismatch(String error, String gradleVersion, JavaHomes.Jdk jdk) {
        if (error == null || jdk == null || JavaHomes.gradleVersion(gradleVersion) == null) {
            return null;
        }
        // A plugin that needs a newer Java than the daemon's ("Android Gradle plugin requires
        // Java 11 to run"): the LTS release at or above it, when the build's Gradle runs on that.
        Matcher needs = PLUGIN_NEEDS_JAVA.matcher(error);
        if (needs.find()) {
            int wanted = Integer.parseInt(needs.group(1));
            int lts = JavaHomes.lts(gradleVersion, JavaHomes.maxJava(gradleVersion));
            for (int candidate : new int[] {8, 11, 17, 21, 25}) {
                if (candidate >= wanted && candidate <= JavaHomes.maxJava(gradleVersion)) {
                    lts = candidate;
                    break;
                }
            }
            if (lts >= wanted && jdk.feature() < wanted) {
                return lts;
            }
        }
        boolean mismatch = GROOVY_ON_NEW_JAVA.matcher(error).find() || error.contains("Unsupported Java")
            || jdk.feature() > JavaHomes.maxJava(gradleVersion) || jdk.feature() < JavaHomes.minJava(gradleVersion);
        return mismatch ? JavaHomes.lts(gradleVersion, JavaHomes.wantedJava(gradleVersion, null)) : null;
    }

    private static final Pattern PLUGIN_NEEDS_JAVA = Pattern.compile("requires Java (\\d+) to run");
    private static final Pattern CLASS_VERSION = Pattern.compile("class file (?:major )?version (\\d+)");
    private static final Pattern PLUGIN_NOT_FOUND = Pattern.compile("Plugin \\[id: '([^']+)'(?:, version: '([^']+)')?[^]]*\\] was not found");
    private static final Pattern REQUIRES_GRADLE = Pattern.compile("[^\\n.]*requires Gradle [^\\n]*");
    private static final Pattern NO_VARIANT = Pattern.compile("Cannot choose between the following variants of ([^\\s:]+:[^\\s:]+:[^\\s:]+?):?\\s");
    private static final Pattern SCRIPT_LOCATION = Pattern.compile("(?:Build file|Settings file|Script) '([^']+)' line: (\\d+)");

    /**
     * A short reason for a failed sync, for a list of failed builds: the cause most likely to
     * explain it, without Gradle's boilerplate.
     */
    static String reason(String error, String gradleVersion, JavaHomes.Jdk jdk) {
        String gradle = "Gradle " + (gradleVersion == null ? "" : gradleVersion);
        Matcher m = REQUIRES_GRADLE.matcher(error);
        if (m.find()) {
            return m.group().trim().replaceAll("\\s+", " ");
        }
        m = CLASS_VERSION.matcher(error);
        if (m.find()) {
            int needed = Integer.parseInt(m.group(1)) - 44;
            return "a class in the build needs Java " + needed + ", but " + gradle.trim()
                + (jdk == null ? "" : " runs on Java " + jdk.feature()) + " here";
        }
        m = PLUGIN_NOT_FOUND.matcher(error);
        if (m.find()) {
            return "plugin " + m.group(1) + (m.group(2) == null ? "" : " " + m.group(2)) + " was not found";
        }
        if (error.contains("Could not install Gradle distribution") || error.contains("Could not download Gradle")) {
            return gradle.trim() + " could not be downloaded";
        }
        if (error.contains("Unsupported Java") || error.contains("Could not determine java version")
            || GROOVY_ON_NEW_JAVA.matcher(error).find() && !error.contains("class file")) {
            return gradle.trim() + " does not run on " + (jdk == null ? "this Java" : "Java " + jdk.feature());
        }
        m = NO_VARIANT.matcher(error);
        if (m.find() && error.contains("configuration ':classpath'")) {
            return "no variant of the build plugin " + m.group(1) + " fits " + gradle.trim()
                + (jdk == null ? "" : " on Java " + jdk.feature()) + "; it needs a newer Gradle or Java";
        }
        // The innermost cause usually says what is wrong; the outer ones say where.
        String[] causes = error.split("\n\\s*caused by: ");
        String[] lines = causes[causes.length - 1].split("\n");
        String last = GradleSaid.saying(lines, 0, lines[0].trim());
        if (causes[0].startsWith(GradleSaid.PREFIX)) {
            // What Gradle printed says more than an exception the Tooling API could not carry.
            last = GradleSaid.deepest(causes[0].substring(GradleSaid.PREFIX.length()));
        }
        m = SCRIPT_LOCATION.matcher(error);
        String where = m.find() ? new File(m.group(1)).getName() + ":" + m.group(2) + ": " : "";
        String reason = where + last;
        return reason.length() > 200 ? reason.substring(0, 197) + "..." : reason;
    }

    /** Whether any build of the model failed. */
    static boolean anyFailed(JsonObject model) {
        for (JsonElement b : model.getAsJsonArray("gradleBuilds")) {
            if ("failed".equals(b.getAsJsonObject().get("status").getAsString())) {
                return true;
            }
        }
        return false;
    }
}
