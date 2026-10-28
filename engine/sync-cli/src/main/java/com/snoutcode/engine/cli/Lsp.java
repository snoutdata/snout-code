package com.snoutcode.engine.cli;

import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

import java.io.File;
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
        File gradleUserHome;
        String gradleVersion;
        boolean scalaBridge = true;
        boolean sources = true;
        boolean offline;
        boolean verbose;
        String maxHeap = "2G";
        final List<String> jvmArgs = new ArrayList<>();
        final List<File> jdks = new ArrayList<>();
        int parallel;
    }

    private Lsp() {
    }

    static int run(Options options, PrintStream log) throws IOException, InterruptedException {
        File projectDir = options.projectDir.getCanonicalFile();
        String hash = Sync.sha256(projectDir.getAbsolutePath().getBytes(StandardCharsets.UTF_8)).substring(0, 16);
        Path cache = (options.cacheDir != null ? options.cacheDir.toPath() : defaultCacheDir()).toAbsolutePath();
        Path home = cache.resolve("jdtls").resolve(hash);
        Files.createDirectories(home);
        Path modelFile = home.resolve("model.json");

        File jdtls = options.jdtlsHome != null ? options.jdtlsHome : installed("jdtls");
        File bundle = options.bundle != null ? options.bundle : installedBundle();
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
        sync.gradleOutput = options.verbose ? log : OutputStream.nullOutputStream();
        project.jdks.addAll(options.jdks);
        project.cacheDir = cache;
        project.log = log;
        if (options.parallel > 0) {
            project.parallel = options.parallel;
        }

        JsonObject model;
        // Each stage announces itself on stderr, for a client that shows progress.
        log.println("snout-engine: syncing " + projectDir);
        try {
            List<ProjectSync.Plan> plans = ProjectSync.plan(project);
            model = ProjectSync.sync(project, plans);
            // A broken build file must not cost the user the editor: a build that fails now keeps
            // its model from the last sync that worked.
            int stale = keepLastGood(model, modelFile);
            if (stale > 0) {
                log.println("snout-engine: using the last good model for " + stale + " build(s) that failed");
            }
            log.println("snout-engine: " + SnoutEngine.summary(model));
            if (options.scalaBridge) {
                compileScala(model, plans, project.parallel, log);
            }
        } catch (IOException | RuntimeException e) {
            log.println("snout-engine: sync failed: " + SnoutEngine.describe(e));
            if (Files.isRegularFile(modelFile)) {
                log.println("snout-engine: using the model from the last sync");
                model = null;
            } else {
                model = new JsonObject();
                model.addProperty("modelVersion", ProjectSync.MODEL_VERSION);
                model.addProperty("rootDir", projectDir.getAbsolutePath());
                model.add("gradleBuilds", new JsonArray());
                model.addProperty("syncError", SnoutEngine.describe(e));
            }
        }
        if (model != null) {
            writeAtomically(modelFile, new GsonBuilder().setPrettyPrinting().disableHtmlEscaping().create().toJson(model));
        }

        Path config = prepareConfiguration(home.resolve("config"), jdtls.toPath(), bundle.toPath());
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
        command.add("-XX:+UseParallelGC");
        command.add("-XX:GCTimeRatio=4");
        command.add("-XX:AdaptiveSizePolicyWeight=90");
        command.add("-Xms100m");
        command.add("-Xmx" + options.maxHeap);
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

        log.println("snout-engine: starting jdtls");
        Process process = new ProcessBuilder(command).inheritIO().start();
        Thread stop = new Thread(process::destroy, "stop-jdtls");
        Runtime.getRuntime().addShutdownHook(stop);
        int exit = process.waitFor();
        try {
            Runtime.getRuntime().removeShutdownHook(stop);
        } catch (IllegalStateException e) {
            // already shutting down
        }
        return exit;
    }

    /**
     * Runs the Scala bridge for every build that synced and has Scala, several builds at once,
     * each with the JDK and caches its sync used.
     */
    private static void compileScala(JsonObject model, List<ProjectSync.Plan> plans, int parallel, PrintStream log)
        throws IOException {
        Map<String, JsonObject> entries = new HashMap<>();
        for (JsonElement g : model.getAsJsonArray("gradleBuilds")) {
            JsonObject entry = g.getAsJsonObject();
            if ("synced".equals(entry.get("status").getAsString()) && hasScala(entry)) {
                entries.put(entry.get("id").getAsString(), entry);
            }
        }
        List<ProjectSync.Plan> scala = plans.stream().filter(p -> entries.containsKey(p.id())).toList();
        if (scala.isEmpty()) {
            return;
        }
        log.println("snout-engine: compiling Scala in " + scala.size() + " build(s)");
        long start = System.nanoTime();
        List<ScalaBridge.Result> results = ProjectSync.parallel(scala, parallel, plan -> {
            try {
                ScalaBridge.Result r = ScalaBridge.compile(entries.get(plan.id()), plan.options());
                log.println("snout-engine: compiled Scala in " + plan.id() + " (" + String.join(" ", r.tasks) + ") in "
                    + r.millis + " ms; " + r.sourceSets + " class folder(s) for jdtls"
                    + (r.error == null ? "" : "; the compile failed: " + r.error));
                return r;
            } catch (IOException | RuntimeException e) {
                ScalaBridge.Result r = new ScalaBridge.Result();
                r.error = SnoutEngine.describe(e);
                log.println("snout-engine: could not compile Scala in " + plan.id() + ": " + r.error);
                return r;
            }
        });
        int folders = results.stream().mapToInt(r -> r.sourceSets).sum();
        long failed = results.stream().filter(r -> r.error != null).count();
        log.println("snout-engine: compiled Scala in " + scala.size() + " build(s) in " + (System.nanoTime() - start) / 1_000_000
            + " ms; " + folders + " class folder(s) for jdtls" + (failed == 0 ? "" : "; " + failed + " build(s) failed to compile"));
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
    static Path prepareConfiguration(Path dir, Path jdtls, Path bundle) throws IOException {
        Path template = jdtls.resolve(platformConfigDir()).resolve("config.ini");
        Properties ini = new Properties();
        try (Reader reader = Files.newBufferedReader(template, StandardCharsets.ISO_8859_1)) {
            ini.load(reader);
        }
        String reference = "reference:file:" + bundle.toAbsolutePath() + "@4";
        ini.setProperty("osgi.bundles", ini.getProperty("osgi.bundles") + "," + reference);
        ini.setProperty("osgi.checkConfiguration", "true");

        String stamp = template.toAbsolutePath() + "\n" + Files.getLastModifiedTime(template) + "\n"
            + bundle.toAbsolutePath() + "\n" + Files.getLastModifiedTime(bundle) + "\n" + Files.size(bundle) + "\n";
        Path stampFile = dir.resolve("snout.stamp");
        if (Files.isRegularFile(stampFile) && Files.readString(stampFile).equals(stamp)) {
            return dir;
        }
        deleteTree(dir);
        Files.createDirectories(dir);
        try (Writer writer = Files.newBufferedWriter(dir.resolve("config.ini"), StandardCharsets.ISO_8859_1)) {
            ini.store(writer, "Generated by snout-engine from " + template);
        }
        Files.writeString(stampFile, stamp);
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

    private static void writeAtomically(Path file, String text) throws IOException {
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
