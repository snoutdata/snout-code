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
    }

    /** One build to sync, with the choices made for it. */
    /** One build to sync; {@code needsJdk} is the Java major it needs when no installed JDK runs its Gradle. */
    public record Plan(String id, File rootDir, String gradleVersion, JavaHomes.Jdk jdk, Sync.Options options, Integer needsJdk) {
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

    /** The builds of the folder, each with its id, Gradle version, JDK and own options. */
    public static List<Plan> plan(Options options) throws IOException {
        File folder = options.sync.projectDir.getCanonicalFile();
        List<JavaHomes.Jdk> installed = JavaHomes.discover(options.jdks, options.cacheDir);
        JavaHomes.Jdk preferred = options.sync.javaHome == null ? null : JavaHomes.read(options.sync.javaHome);
        List<Plan> plans = new ArrayList<>();
        for (File root : Discovery.builds(folder)) {
            String id = buildId(folder, root);
            String version = options.sync.gradleVersion != null ? options.sync.gradleVersion : JavaHomes.wrapperVersion(root);
            Integer level = JavaHomes.scriptLevel(root);
            JavaHomes.Jdk jdk = JavaHomes.choose(version, level, preferred, installed);
            Sync.Options o = copy(options.sync);
            o.projectDir = root;
            o.javaHome = jdk != null ? jdk.home() : options.sync.javaHome;
            if (options.sync.projectCacheDir != null) {
                o.projectCacheDir = new File(options.sync.projectCacheDir, key(root));
            }
            plans.add(new Plan(id, root, version, jdk, o, JavaHomes.needs(version, level, jdk)));
        }
        return plans;
    }

    /** One build's entry: its sync model, or what went wrong. Never throws. */
    static JsonObject syncOne(Plan plan, PrintStream log) {
        long start = System.nanoTime();
        JsonObject entry;
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
        return o;
    }

    // ---- why a build failed, in a few words

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
        if (error.contains("Unsupported Java") || error.contains("Could not determine java version")) {
            return gradle.trim() + " does not run on " + (jdk == null ? "this Java" : "Java " + jdk.feature());
        }
        m = NO_VARIANT.matcher(error);
        if (m.find() && error.contains("configuration ':classpath'")) {
            return "no variant of the build plugin " + m.group(1) + " fits " + gradle.trim()
                + (jdk == null ? "" : " on Java " + jdk.feature()) + "; it needs a newer Gradle or Java";
        }
        // The innermost cause usually says what is wrong; the outer ones say where.
        String[] causes = error.split("\n\\s*caused by: ");
        String last = causes[causes.length - 1].split("\n")[0].trim();
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
