package com.snoutcode.engine.cli;

import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

import java.io.BufferedOutputStream;
import java.io.File;
import java.io.FileDescriptor;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.OutputStream;
import java.io.PrintStream;
import java.io.Reader;
import java.io.Writer;
import java.net.URISyntaxException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Properties;
import java.util.Set;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.stream.Stream;

/**
 * {@code snout-engine lsp <folder>}: syncs the Gradle builds of the folder into one model (see
 * {@link ProjectSync}), compiles the Scala the Java code needs, then runs jdtls with the SnoutCode importer bundle and hands it this process's stdin and
 * stdout, so the caller talks LSP to jdtls directly.
 *
 * <p>Nothing is written into the project: Gradle's build directories and project cache, the
 * model and jdtls's configuration and workspace all live under the cache directory, by default
 * {@code ~/Library/Caches/SnoutCode} on macOS:
 *
 * <pre>
 *   build/&lt;hash&gt;/          Gradle build directories (the Scala classes)
 *   gradle/&lt;hash&gt;/&lt;build&gt;/ Gradle project caches (.gradle), one per build
 *   jdtls/&lt;hash&gt;/model.json   the synced model the importer reads
 *   jdtls/&lt;hash&gt;/config/  OSGi configuration
 *   jdtls/&lt;hash&gt;/workspace/  jdtls workspace data: JDT projects, indexes, output folders
 * </pre>
 *
 * where the hash identifies the folder.
 */
final class Lsp {
    /** Options of {@code snout-engine lsp}. */
    static final class Options {
        File projectDir;
        File javaHome;
        File jdtlsJavaHome;
        File cacheDir;
        File jdtlsHome;
        File bundle;
        File debugBundle;
        boolean debugger = true;
        File springBundle;
        boolean spring = true;
        File gradleUserHome;
        String gradleVersion;
        boolean scalaBridge = true;
        boolean sources = true;
        boolean offline;
        boolean verbose;
        /** jdtls's {@code -Xmx}, or {@code auto}: by the project's size (see {@link #autoHeap}). */
        String maxHeap = "auto";
        final List<String> jvmArgs = new ArrayList<>();
        final List<File> jdks = new ArrayList<>();
        int parallel;
    }

    private Lsp() {
    }

    static int run(Options options, PrintStream log) throws IOException, InterruptedException {
        // The Gradle daemons the engine starts are its own (see Daemons), and end with it: when
        // the session ends, and on any other exit (the app gone, a signal) through the hook.
        File registry = Daemons.registry(cacheDir(options), options.projectDir.getCanonicalFile());
        AtomicBoolean stopped = new AtomicBoolean();
        Runnable stopDaemons = () -> {
            if (stopped.compareAndSet(false, true)) {
                Daemons.stop(registry, DAEMON_GRACE_MILLIS, log);
            }
        };
        Thread hook = new Thread(stopDaemons, "stop-gradle-daemons");
        Runtime.getRuntime().addShutdownHook(hook);
        try {
            return session(options, log, registry);
        } finally {
            stopDaemons.run();
            try {
                Runtime.getRuntime().removeShutdownHook(hook);
            } catch (IllegalStateException e) {
                // already shutting down
            }
        }
    }

    /** How long the daemons get to end on their own before they are killed. */
    static final long DAEMON_GRACE_MILLIS = 3000;

    private static Path cacheDir(Options options) {
        return (options.cacheDir != null ? options.cacheDir.toPath() : defaultCacheDir()).toAbsolutePath();
    }

    private static int session(Options options, PrintStream log, File daemonRegistry) throws IOException, InterruptedException {
        // Standard output carries LSP alone: keep anything else that prints off it.
        BufferedOutputStream appOut = new BufferedOutputStream(new FileOutputStream(FileDescriptor.out));
        System.setOut(log);
        Readiness readiness = new Readiness(message -> LspProxy.write(appOut, message));
        File projectDir = options.projectDir.getCanonicalFile();
        String hash = Sync.sha256(projectDir.getAbsolutePath().getBytes(StandardCharsets.UTF_8)).substring(0, 16);
        Path cache = cacheDir(options);
        Path home = cache.resolve("jdtls").resolve(hash);
        Files.createDirectories(home);
        Path modelFile = home.resolve("model.json");
        Path inputsFile = home.resolve("model-inputs.json");
        Path bridgeState = home.resolve("scala-bridge.json");

        File jdtls = options.jdtlsHome != null ? options.jdtlsHome : installed("jdtls");
        File bundle = options.bundle != null ? options.bundle : installedBundle();
        File debugBundle = !options.debugger ? null : options.debugBundle != null ? options.debugBundle : installedDebugBundle();
        File java = javaExecutable(options.jdtlsJavaHome);
        checkJava(java, options.jdtlsJavaHome);

        ProjectSync.Options project = new ProjectSync.Options();
        Sync.Options sync = project.sync;
        sync.projectDir = projectDir;
        sync.javaHome = options.javaHome;
        sync.gradleUserHome = options.gradleUserHome;
        sync.gradleVersion = options.gradleVersion;
        sync.sources = options.sources;
        sync.offline = options.offline;
        sync.projectCacheDir = cache.resolve("gradle").resolve(hash).toFile();
        sync.buildRoot = cache.resolve("build").resolve(hash).toFile();
        sync.bridgeRoot = cache.resolve("bridge").resolve(hash).toFile();
        sync.daemonRegistry = daemonRegistry;
        sync.gradleOutput = options.verbose ? log : OutputStream.nullOutputStream();
        project.jdks.addAll(options.jdks);
        project.cacheDir = cache;
        project.log = log;
        if (options.parallel > 0) {
            project.parallel = options.parallel;
        }
        String optionsKey = optionsKey(options, sync);

        JsonObject model;
        List<ProjectSync.Plan> plans = List.of();
        JsonObject inputs = null;
        Set<String> uncertain = Set.of();
        try {
            plans = ProjectSync.plan(project);
            // A start whose builds did not change since the last sync takes that sync's model.
            JsonObject cached = readModel(modelFile);
            String changed = cached == null ? null : ModelCache.changed(ModelCache.read(inputsFile), plans, optionsKey);
            if (cached != null && changed == null) {
                model = cached;
                uncertain = ModelCache.uncertain(model);
                log.println("snout-engine: the builds did not change since the last sync; using its model"
                    + (uncertain.isEmpty() ? "" : " (" + uncertain.size() + " build(s) sync again in the background)"));
            } else {
                // Each stage announces itself on stderr, for a client that shows progress.
                log.println("snout-engine: syncing " + projectDir + (changed == null ? "" : " (" + changed + ")"));
                readiness.syncing();
                model = ProjectSync.sync(project, plans);
                // A broken build file must not cost the user the editor: a build that fails now keeps
                // its model from the last sync that worked.
                int stale = keepLastGood(model, modelFile);
                if (stale > 0) {
                    log.println("snout-engine: using the last good model for " + stale + " build(s) that failed");
                }
                inputs = ModelCache.inputs(model, plans, optionsKey, options.gradleUserHome);
            }
            log.println("snout-engine: " + SnoutEngine.summary(model));
        } catch (IOException | RuntimeException e) {
            log.println("snout-engine: sync failed: " + SnoutEngine.describe(e));
            model = readModel(modelFile);
            if (model != null) {
                log.println("snout-engine: using the model from the last sync");
            } else {
                model = new JsonObject();
                model.addProperty("modelVersion", ProjectSync.MODEL_VERSION);
                model.addProperty("rootDir", projectDir.getAbsolutePath());
                model.add("gradleBuilds", new JsonArray());
                model.addProperty("syncError", SnoutEngine.describe(e));
            }
        }
        // The Scala bridge's class folders go into the model before jdtls starts, empty when
        // nothing compiled them yet: jdtls imports them now and reads the classes once compiled.
        ScalaBridge.Work bridge = null;
        if (options.scalaBridge) {
            try {
                bridge = ScalaBridge.plan(model, plans, bridgeState);
                ScalaBridge.annotate(bridge);
            } catch (IOException | RuntimeException e) {
                log.println("snout-engine: could not plan the Scala bridge: " + SnoutEngine.describe(e));
                bridge = null;
            }
        }
        writeAtomically(modelFile, new GsonBuilder().setPrettyPrinting().disableHtmlEscaping().create().toJson(model));
        if (inputs != null) {
            ModelCache.write(inputsFile, inputs);
        }
        readiness.model(model);

        List<Path> bundles = new ArrayList<>();
        bundles.add(bundle.toPath());
        if (debugBundle != null) {
            bundles.add(debugBundle.toPath());
        }
        File springBundle = !options.spring ? null : options.springBundle != null ? options.springBundle : installedSpringBundle();
        if (springBundle != null) {
            bundles.add(springBundle.toPath());
        }
        Path config = prepareConfiguration(home.resolve("config"), jdtls.toPath(), bundles);
        List<String> command = new ArrayList<>();
        command.add(java.getAbsolutePath());
        command.add("-Declipse.application=org.eclipse.jdt.ls.core.id1");
        command.add("-Dosgi.bundles.defaultStartLevel=4");
        command.add("-Declipse.product=org.eclipse.jdt.ls.core.product");
        command.add("-Dosgi.checkConfiguration=true");
        // Should Buildship or m2e ever import a project we do not model, keep its metadata out too.
        command.add("-Djava.import.generatesMetadataFilesAtProjectRoot=false");
        command.add("-Dsnout.model=" + modelFile);
        command.add("-Dsun.zip.disableMemoryMapping=true");
        command.addAll(heapArguments(options.maxHeap, model, options.jvmArgs));
        command.add("--add-modules=ALL-SYSTEM");
        command.add("--add-opens");
        command.add("java.base/java.util=ALL-UNNAMED");
        command.add("--add-opens");
        command.add("java.base/java.lang=ALL-UNNAMED");
        command.addAll(options.jvmArgs);
        command.add("-jar");
        command.add(launcherJar(jdtls.toPath()).toString());
        command.add("-configuration");
        command.add(config.toString());
        command.add("-data");
        command.add(home.resolve("workspace").toString());

        // A jdtls left on this workspace by an engine that is gone (killed, or crashed with the
        // app) would hold its lock, and the new one could not start.
        Leftovers.stopJdtls(home.resolve("workspace"), log);
        log.println("snout-engine: starting jdtls (" + command.stream().filter(a -> a.startsWith("-Xmx")).reduce((a, b) -> b).orElse("")
            + ")");
        Process process = new ProcessBuilder(command).redirectError(ProcessBuilder.Redirect.INHERIT).start();
        LspProxy proxy = new LspProxy(System.in, appOut, process, log);
        Engine engine = new Engine(model, plans, proxy::toApp, log, home.resolve("runs"));
        engine.attach(proxy);
        engine.attach(readiness);
        engine.modelFile(modelFile);
        engine.bridgeState(bridgeState);
        engine.verbose(options.verbose);
        engine.bridge(bridge);
        proxy.attach(engine);
        SpringSupport spring = springBundle == null ? null : new SpringSupport(proxy, proxy::toApp, log);
        if (spring != null) {
            proxy.attach(spring);
            engine.attach(spring);
        }
        // The Scala bridge compiles while jdtls starts; a module is ready once jdtls has its classes.
        if (bridge != null) {
            engine.startScala(project.parallel);
        }
        if (!uncertain.isEmpty()) {
            JsonObject synced = model;
            List<ProjectSync.Plan> planned = plans;
            engine.resyncLater(uncertain, project, () -> {
                try {
                    ModelCache.write(inputsFile, ModelCache.inputs(synced, planned, optionsKey, options.gradleUserHome));
                } catch (IOException e) {
                    log.println("snout-engine: could not record the sync's inputs: " + SnoutEngine.describe(e));
                }
            });
        }
        Thread stop = new Thread(() -> {
            engine.shutdown();
            stopProcess(process, 5);
        }, "stop-jdtls");
        Runtime.getRuntime().addShutdownHook(stop);
        int exit = proxy.run();
        engine.shutdown();
        if (spring != null) {
            spring.shutdown();
        }
        try {
            Runtime.getRuntime().removeShutdownHook(stop);
        } catch (IllegalStateException e) {
            // already shutting down
        }
        return exit;
    }

    /**
     * End this engine when the process that started it (the app) ends, however it ends: killed,
     * crashed, or quit before it could say so. The shutdown hook then stops jdtls and the runs.
     * Without this, an engine still syncing (not yet reading its stdin) would outlive the app.
     */
    static void exitWithParent(PrintStream log) {
        ProcessHandle parent = ProcessHandle.current().parent().orElse(null);
        if (parent == null || parent.pid() <= 1) {
            return;
        }
        parent.onExit().thenRun(() -> {
            log.println("snout-engine: the app (pid " + parent.pid() + ") is gone; stopping");
            System.exit(0);
        });
    }

    /** Ask {@code process} to end (SIGTERM), and make it after {@code graceSeconds}. */
    static void stopProcess(Process process, int graceSeconds) {
        process.destroy();
        try {
            if (!process.waitFor(graceSeconds, java.util.concurrent.TimeUnit.SECONDS)) {
                process.destroyForcibly();
            }
        } catch (InterruptedException e) {
            process.destroyForcibly();
            Thread.currentThread().interrupt();
        }
    }

    /** The model written last, or null when there is none or it is of another format. */
    private static JsonObject readModel(Path file) {
        try {
            if (!Files.isRegularFile(file)) {
                return null;
            }
            JsonObject model = JsonParser.parseString(Files.readString(file)).getAsJsonObject();
            return model.has("modelVersion") && model.get("modelVersion").getAsInt() == ProjectSync.MODEL_VERSION
                && model.has("gradleBuilds") ? model : null;
        } catch (IOException | RuntimeException e) {
            return null;
        }
    }

    /**
     * What a cached model depends on besides the build files: the options that change a sync, and
     * the sync plugin and model format of this engine.
     */
    static String optionsKey(Options options, Sync.Options sync) {
        StringBuilder s = new StringBuilder();
        s.append("model ").append(ProjectSync.MODEL_VERSION).append(' ')
            .append(com.snoutcode.engine.sync.ProjectModelBuilder.MODEL_VERSION).append('\n');
        File plugin = Sync.pluginClasspath();
        s.append(plugin.getAbsolutePath()).append(' ').append(plugin.length()).append(' ').append(plugin.lastModified()).append('\n');
        s.append(sync.javaHome).append('\n').append(options.jdks).append('\n').append(sync.gradleUserHome).append('\n')
            .append(sync.gradleVersion).append('\n').append(sync.sources).append(' ').append(sync.offline).append('\n')
            .append(sync.buildRoot).append('\n');
        return Sync.sha256(s.toString().getBytes(StandardCharsets.UTF_8));
    }

    /**
     * jdtls's heap: {@code -Xmx} as given, or by the project's size when {@code auto}; and the
     * parallel collector (as vscode-java runs jdtls), unless the JVM arguments name a collector.
     * G1 was measured on a large project: a higher peak and no faster.
     */
    static List<String> heapArguments(String maxHeap, JsonObject model, List<String> jvmArgs) {
        List<String> out = new ArrayList<>();
        if (jvmArgs.stream().noneMatch(a -> a.matches("-XX:\\+Use\\w+GC"))) {
            out.add("-XX:+UseParallelGC");
            out.add("-XX:GCTimeRatio=4");
            out.add("-XX:AdaptiveSizePolicyWeight=90");
        }
        out.add("-Xms100m");
        out.add("-Xmx" + (maxHeap == null || maxHeap.equals("auto") ? autoHeap(model) : maxHeap));
        return out;
    }

    /**
     * A heap limit by the project's size: 512 MB, plus 150 MB per thousand Java and Scala source
     * files and half a megabyte per library, rounded up to 256 MB; at least 1 GB, at most 8 GB
     * or a quarter of the machine's memory. A project of 3,400 source files and 540 libraries
     * gets 1.5 GB: jdtls's live data peaked at 0.6 GB there while it imported, indexed and built
     * everything, and 1.2 GB ran as fast as 2 GB with 0.6 GB less resident.
     */
    static String autoHeap(JsonObject model) {
        long files = 0;
        Set<String> dirs = new java.util.LinkedHashSet<>();
        for (JsonElement m : model.has("modules") ? model.getAsJsonArray("modules") : new JsonArray()) {
            JsonObject module = m.getAsJsonObject();
            for (JsonElement r : module.has("sourceRoots") ? module.getAsJsonArray("sourceRoots") : new JsonArray()) {
                JsonObject root = r.getAsJsonObject();
                if (root.has("dir") && !root.has("owner") && !(root.has("shared") && root.get("shared").getAsBoolean())
                    && !String.valueOf(root.get("type")).contains("resources")) {
                    dirs.add(root.get("dir").getAsString());
                }
            }
        }
        for (JsonElement r : model.has("sharedSourceRoots") ? model.getAsJsonArray("sharedSourceRoots") : new JsonArray()) {
            JsonObject root = r.getAsJsonObject();
            if (root.has("dir") && !String.valueOf(root.get("type")).contains("resources")) {
                dirs.add(root.get("dir").getAsString());
            }
        }
        for (String dir : dirs) {
            Path d = Path.of(dir);
            if (!Files.isDirectory(d)) {
                continue;
            }
            try (Stream<Path> walk = Files.walk(d)) {
                files += walk.filter(f -> {
                    String n = f.getFileName().toString();
                    return n.endsWith(".java") || n.endsWith(".scala");
                }).count();
            } catch (IOException | RuntimeException e) {
                // a folder we cannot read adds nothing
            }
        }
        int libraries = model.has("libraries") ? model.getAsJsonArray("libraries").size() : 0;
        long mb = 512 + files * 150 / 1000 + libraries / 2;
        mb = (mb + 255) / 256 * 256;
        long physical = physicalMemoryMb();
        long max = physical > 0 ? Math.min(8192, physical / 4) : 8192;
        mb = Math.max(1024, Math.min(max, mb));
        return mb + "m";
    }

    private static long physicalMemoryMb() {
        try {
            if (java.lang.management.ManagementFactory.getOperatingSystemMXBean() instanceof com.sun.management.OperatingSystemMXBean os) {
                return os.getTotalMemorySize() / (1024 * 1024);
            }
        } catch (RuntimeException | LinkageError e) {
            // unknown
        }
        return -1;
    }

    /**
     * Replaces each failed build of the model with its entry from the last model written, when
     * that sync worked, marked stale and carrying the new error. Returns how many it replaced.
     */
    static int keepLastGood(JsonObject model, Path lastModel) {
        if (!Files.isRegularFile(lastModel)) {
            return 0;
        }
        Map<String, JsonObject> previous = new HashMap<>();
        try {
            JsonObject last = JsonParser.parseString(Files.readString(lastModel)).getAsJsonObject();
            for (JsonElement g : last.has("gradleBuilds") ? last.getAsJsonArray("gradleBuilds") : new JsonArray()) {
                JsonObject entry = g.getAsJsonObject();
                if (entry.has("status") && !"failed".equals(entry.get("status").getAsString()) && entry.has("rootDir")) {
                    previous.put(entry.get("rootDir").getAsString(), entry);
                }
            }
        } catch (IOException | RuntimeException e) {
            return 0;
        }
        int replaced = 0;
        JsonArray builds = model.getAsJsonArray("gradleBuilds");
        for (int i = 0; i < builds.size(); i++) {
            JsonObject entry = builds.get(i).getAsJsonObject();
            JsonObject last = previous.get(entry.get("rootDir").getAsString());
            if (!"failed".equals(entry.get("status").getAsString()) || last == null) {
                continue;
            }
            JsonObject stale = last.deepCopy();
            stale.addProperty("id", entry.get("id").getAsString());
            stale.addProperty("status", "stale");
            stale.add("error", entry.get("error"));
            stale.add("reason", entry.get("reason"));
            if (entry.has("needsJdk")) {
                stale.add("needsJdk", entry.get("needsJdk"));
            }
            builds.set(i, stale);
            replaced++;
        }
        if (replaced > 0) {
            ProjectModel.assemble(model);
        }
        return replaced;
    }

    /** Whether any source set of a Gradle build's model has Scala sources for the bridge to compile. */
    static boolean hasScala(JsonObject model) {
        for (var b : model.getAsJsonArray("builds")) {
            for (var p : b.getAsJsonObject().getAsJsonArray("projects")) {
                JsonObject project = p.getAsJsonObject();
                if (!project.has("sourceSets")) {
                    continue;
                }
                for (var s : project.getAsJsonArray("sourceSets")) {
                    JsonObject sources = s.getAsJsonObject().getAsJsonObject("sources");
                    if (sources != null && sources.has("scala")) {
                        return true;
                    }
                }
            }
        }
        return false;
    }

    /** {@code ~/Library/Caches/SnoutCode} on macOS, the XDG cache or LOCALAPPDATA elsewhere. */
    static Path defaultCacheDir() {
        String os = System.getProperty("os.name", "").toLowerCase(Locale.ROOT);
        Path userHome = Path.of(System.getProperty("user.home"));
        if (os.contains("mac")) {
            return userHome.resolve("Library/Caches/SnoutCode");
        }
        if (os.contains("win")) {
            String local = System.getenv("LOCALAPPDATA");
            return (local != null ? Path.of(local) : userHome.resolve("AppData/Local")).resolve("SnoutCode/Cache");
        }
        String xdg = System.getenv("XDG_CACHE_HOME");
        return (xdg != null && !xdg.isEmpty() ? Path.of(xdg) : userHome.resolve(".cache")).resolve("SnoutCode");
    }

    /**
     * A private OSGi configuration: jdtls's own {@code config.ini} for this platform with our
     * bundle added to {@code osgi.bundles}. Rebuilt from scratch when jdtls or the bundle changes,
     * since Equinox caches the bundles it installed.
     */
    static Path prepareConfiguration(Path dir, Path jdtls, List<Path> bundles) throws IOException {
        Path template = jdtls.resolve(platformConfigDir()).resolve("config.ini");
        Properties ini = new Properties();
        try (Reader reader = Files.newBufferedReader(template, StandardCharsets.ISO_8859_1)) {
            ini.load(reader);
        }
        StringBuilder osgiBundles = new StringBuilder(ini.getProperty("osgi.bundles"));
        StringBuilder stamp = new StringBuilder(template.toAbsolutePath() + "\n" + Files.getLastModifiedTime(template) + "\n");
        for (Path bundle : bundles) {
            osgiBundles.append(",reference:file:").append(bundle.toAbsolutePath()).append("@4");
            stamp.append(bundle.toAbsolutePath()).append('\n').append(Files.getLastModifiedTime(bundle)).append('\n')
                .append(Files.size(bundle)).append('\n');
        }
        ini.setProperty("osgi.bundles", osgiBundles.toString());
        ini.setProperty("osgi.checkConfiguration", "true");
        Path stampFile = dir.resolve("snout.stamp");
        if (Files.isRegularFile(stampFile) && Files.readString(stampFile).equals(stamp.toString())) {
            return dir;
        }
        deleteTree(dir);
        Files.createDirectories(dir);
        try (Writer writer = Files.newBufferedWriter(dir.resolve("config.ini"), StandardCharsets.ISO_8859_1)) {
            ini.store(writer, "Generated by snout-engine from " + template);
        }
        Files.writeString(stampFile, stamp.toString());
        return dir;
    }

    private static String platformConfigDir() {
        String os = System.getProperty("os.name", "").toLowerCase(Locale.ROOT);
        String arch = System.getProperty("os.arch", "").toLowerCase(Locale.ROOT);
        boolean arm = arch.equals("aarch64") || arch.equals("arm64");
        if (os.contains("mac")) {
            return arm ? "config_mac_arm" : "config_mac";
        }
        if (os.contains("win")) {
            return "config_win";
        }
        return arm ? "config_linux_arm" : "config_linux";
    }

    private static Path launcherJar(Path jdtls) throws IOException {
        try (Stream<Path> files = Files.list(jdtls.resolve("plugins"))) {
            return files.filter(f -> f.getFileName().toString().matches("org\\.eclipse\\.equinox\\.launcher_.*\\.jar"))
                .findFirst()
                .orElseThrow(() -> new IOException("no Equinox launcher in " + jdtls.resolve("plugins")));
        }
    }

    /** A directory of the snout-engine installation: the parent of the {@code lib} folder. */
    static File installed(String name) {
        try {
            File jar = new File(Lsp.class.getProtectionDomain().getCodeSource().getLocation().toURI());
            File dir = new File(jar.getParentFile().getParentFile(), name);
            if (!dir.exists()) {
                throw new IllegalStateException("not found: " + dir + " (run from an installed snout-engine, or pass the location)");
            }
            return dir;
        } catch (URISyntaxException e) {
            throw new IllegalStateException(e);
        }
    }

    private static File installedBundle() {
        File[] jars = installed("jdtls-plugins").listFiles((d, n) -> n.startsWith("com.snoutcode.engine.jdtls_") && n.endsWith(".jar"));
        if (jars == null || jars.length != 1) {
            throw new IllegalStateException("expected one importer bundle in " + installed("jdtls-plugins"));
        }
        return jars[0];
    }

    /** The Spring bundle, when the distribution carries it; null otherwise (no Spring support). */
    private static File installedSpringBundle() {
        File[] jars = installed("jdtls-plugins").listFiles((d, n) -> n.startsWith("com.snoutcode.engine.spring_") && n.endsWith(".jar"));
        return jars == null || jars.length == 0 ? null : jars[0];
    }

    /** java-debug's bundle, when the distribution carries it; null otherwise (no debugger). */
    private static File installedDebugBundle() {
        File[] jars = installed("jdtls-plugins").listFiles((d, n) -> n.startsWith("com.microsoft.java.debug.plugin-") && n.endsWith(".jar"));
        return jars == null || jars.length == 0 ? null : jars[0];
    }

    private static File javaExecutable(File javaHome) {
        File home = javaHome != null ? javaHome : new File(System.getProperty("java.home"));
        boolean windows = System.getProperty("os.name", "").toLowerCase(Locale.ROOT).contains("win");
        return new File(home, "bin/" + (windows ? "java.exe" : "java"));
    }

    /** jdtls needs Java 21; say so up front instead of letting OSGi fail to resolve. */
    private static void checkJava(File java, File javaHome) throws IOException {
        if (!java.canExecute()) {
            throw new IOException("no java at " + java);
        }
        if (javaHome == null && Runtime.version().feature() < 21) {
            throw new IOException("jdtls needs Java 21 or newer; this is Java " + Runtime.version().feature()
                + ". Run snout-engine on Java 21 or pass --jdtls-java-home.");
        }
    }

    static void writeAtomically(Path file, String text) throws IOException {
        Path temp = Files.createTempFile(file.getParent(), file.getFileName().toString(), ".tmp");
        Files.writeString(temp, text + "\n", StandardCharsets.UTF_8);
        Files.move(temp, file, StandardCopyOption.REPLACE_EXISTING, StandardCopyOption.ATOMIC_MOVE);
    }

    private static void deleteTree(Path dir) throws IOException {
        if (!Files.exists(dir)) {
            return;
        }
        try (Stream<Path> walk = Files.walk(dir)) {
            for (Path p : walk.sorted(Comparator.reverseOrder()).toList()) {
                Files.delete(p);
            }
        }
    }
}
