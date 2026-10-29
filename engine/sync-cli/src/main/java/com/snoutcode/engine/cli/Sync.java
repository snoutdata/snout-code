package com.snoutcode.engine.cli;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.snoutcode.engine.sync.ProjectModelBuilder;
import com.snoutcode.engine.sync.model.SnoutProjectModel;
import com.snoutcode.engine.sync.action.SyncAction;
import com.snoutcode.engine.sync.action.SyncResult;
import org.gradle.tooling.BuildActionExecuter;
import org.gradle.tooling.GradleConnector;
import org.gradle.tooling.ProjectConnection;

import java.io.File;
import java.io.IOException;
import java.io.OutputStream;
import java.net.URISyntaxException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.ArrayList;
import java.util.HexFormat;
import java.util.List;

/** One sync: connect, inject the init script, run {@link SyncAction}, assemble the JSON model. */
public final class Sync {
    /**
     * The init plugin, by name: loading the class here would need the Gradle API, which only the
     * daemon has. (Constants from ProjectModelBuilder are compile-time and do not load it.)
     */
    static final String PLUGIN_CLASS = "com.snoutcode.engine.sync.SnoutSyncPlugin";

    /** Options of {@code snout-engine sync}. */
    public static final class Options {
        public File projectDir;
        public File javaHome;
        public File gradleUserHome;
        public File projectCacheDir;
        public String gradleVersion;
        public boolean sources = true;
        public boolean offline;
        public OutputStream gradleOutput;
        /** When set, every project's build directory moves under here; see {@link #buildDirInitScript}. */
        public File buildRoot;
        /**
         * The Scala bridge's own build directories, for builds that compile shared Scala folders
         * against one compile of them (see {@link ScalaBridge}); null to use {@link #buildRoot}.
         */
        public File bridgeRoot;
        /**
         * The registry of the engine's own Gradle daemons (see {@link Daemons}); null for the
         * user's, in the Gradle user home.
         */
        public File daemonRegistry;
    }

    private Sync() {
    }

    public static JsonObject run(Options options) throws IOException {
        long start = System.nanoTime();
        File projectDir = options.projectDir.getCanonicalFile();
        List<Path> initScript = new ArrayList<>();
        initScript.add(writeInitScript(pluginClasspath()));
        if (options.buildRoot != null) {
            initScript.add(buildDirInitScript(options.buildRoot));
        }
        SyncResult result = runAction(projectDir, options, initScript, options.projectCacheDir);
        JsonObject model = new JsonObject();
        model.addProperty("modelVersion", ProjectModelBuilder.MODEL_VERSION);
        model.addProperty("rootDir", projectDir.getAbsolutePath());
        model.addProperty("gradleVersion", result.gradleVersion);
        model.addProperty("gradleJavaHome", result.gradleJavaHome);
        JsonArray builds = new JsonArray();
        boolean haveBuildSrc = false;
        for (SyncResult.Build build : result.builds) {
            builds.add(toJson(build));
            haveBuildSrc |= "buildSrc".equals(build.kind);
        }
        // Before Gradle 8, buildSrc is not part of the composite the Tooling API exposes; it is
        // a standalone build, so sync it separately.
        File buildSrc = new File(projectDir, "buildSrc");
        if (!haveBuildSrc && buildSrc.isDirectory()) {
            File cache = options.projectCacheDir == null ? null : new File(options.projectCacheDir, "buildSrc");
            try {
                SyncResult extra = runAction(buildSrc, options, initScript, cache);
                for (SyncResult.Build build : extra.builds) {
                    build.kind = "buildSrc";
                    builds.add(toJson(build));
                }
            } catch (RuntimeException e) {
                JsonObject failed = new JsonObject();
                failed.addProperty("kind", "buildSrc");
                failed.addProperty("rootDir", buildSrc.getAbsolutePath());
                failed.add("projects", new JsonArray());
                JsonArray errors = new JsonArray();
                errors.add(SnoutEngine.describe(e));
                failed.add("errors", errors);
                builds.add(failed);
            }
        }
        model.add("builds", builds);
        model.addProperty("syncMillis", (System.nanoTime() - start) / 1_000_000);
        return model;
    }

    /** A connector for this build, with the user home and Gradle version the options ask for. */
    static GradleConnector connector(File dir, Options options) {
        GradleConnector connector = GradleConnector.newConnector().forProjectDirectory(dir);
        if (options.gradleUserHome != null) {
            connector.useGradleUserHomeDir(options.gradleUserHome);
        }
        if (options.gradleVersion != null) {
            connector.useGradleVersion(options.gradleVersion);
        }
        File registry = options.daemonRegistry != null ? options.daemonRegistry : Daemons.fallbackRegistry();
        if (registry != null) {
            Daemons.configure(connector, registry);
        }
        return connector;
    }

    /** The arguments every Gradle run of ours shares: init scripts, project cache, offline. */
    static List<String> arguments(List<Path> initScripts, File projectCacheDir, Options options) {
        List<String> args = new ArrayList<>();
        for (Path script : initScripts) {
            args.add("--init-script");
            args.add(script.toAbsolutePath().toString());
        }
        if (projectCacheDir != null) {
            args.add("--project-cache-dir");
            args.add(projectCacheDir.getAbsolutePath());
        }
        if (options.offline) {
            args.add("--offline");
        }
        return args;
    }

    private static SyncResult runAction(File dir, Options options, List<Path> initScripts, File projectCacheDir) {
        try (ProjectConnection connection = connector(dir, options).connect()) {
            List<String> args = arguments(initScripts, projectCacheDir, options);
            args.add("-D" + ProjectModelBuilder.SOURCES_PROPERTY + "=" + options.sources);
            BuildActionExecuter<SyncResult> action = connection.action(new SyncAction()).withArguments(args);
            if (options.javaHome != null) {
                action.setJavaHome(options.javaHome);
            }
            if (options.gradleOutput != null) {
                action.setStandardOutput(options.gradleOutput);
                action.setStandardError(options.gradleOutput);
            }
            return action.run();
        }
    }

    private static JsonObject toJson(SyncResult.Build build) {
        JsonObject out = new JsonObject();
        out.addProperty("name", build.name);
        out.addProperty("kind", build.kind);
        out.addProperty("rootDir", build.rootDir);
        JsonArray projects = new JsonArray();
        for (String json : build.projectJson) {
            projects.add(JsonParser.parseString(json));
        }
        out.add("projects", projects);
        JsonArray errors = new JsonArray();
        build.errors.forEach(errors::add);
        out.add("errors", errors);
        return out;
    }

    /** The jar (or classes directory, in development) that holds the model builders. */
    static File pluginClasspath() {
        try {
            return new File(SnoutProjectModel.class.getProtectionDomain().getCodeSource().getLocation().toURI());
        } catch (URISyntaxException e) {
            throw new IllegalStateException("cannot locate the sync plugin", e);
        }
    }

    /**
     * The init script: puts the plugin on the init script classpath and applies it to every build
     * of the composite. The file name prefix lets the model skip it in its list of build scripts.
     *
     * <p>The path is stable for a given plugin location (a content hash), so Gradle compiles the
     * script once and reuses its script cache on later syncs.
     */
    static Path writeInitScript(File pluginClasspath) throws IOException {
        String text = "// Generated by snout-engine sync.\n"
            + "initscript {\n"
            + "    dependencies {\n"
            + "        classpath files(" + groovyString(pluginClasspath.getAbsolutePath()) + ")\n"
            + "    }\n"
            + "}\n"
            + "apply plugin: " + PLUGIN_CLASS + "\n";
        return writeScript("snout-sync-init", text);
    }

    /**
     * An init script that moves the build directory of every project, in every build of the
     * composite, to {@code <buildRoot>/<project name>-<hash of the project directory>}. With it,
     * a sync and a compile leave the user's repository untouched, and the model reports the
     * output folders where they really are.
     */
    static Path buildDirInitScript(File buildRoot) throws IOException {
        String text = "// Generated by snout-engine: keeps build outputs out of the project directory.\n"
            + "def snoutBuildRoot = new File(" + groovyString(buildRoot.getAbsolutePath()) + ")\n"
            + "gradle.allprojects { p ->\n"
            + "    def digest = java.security.MessageDigest.getInstance('SHA-256')\n"
            + "    def key = digest.digest(p.projectDir.absolutePath.getBytes('UTF-8')).encodeHex().toString().substring(0, 12)\n"
            + "    def dir = new File(snoutBuildRoot, p.name.replaceAll(/[^A-Za-z0-9._-]/, '_') + '-' + key)\n"
            + "    try {\n"
            + "        p.layout.buildDirectory.set(dir)\n"
            + "    } catch (Throwable ignored) {\n"
            + "        p.buildDir = dir\n"
            + "    }\n"
            + "}\n";
        return writeScript("snout-build-dirs", text);
    }

    /** Writes a script under a content-hashed name, once, so Gradle's script cache stays warm. */
    static Path writeScript(String prefix, String text) throws IOException {
        byte[] bytes = text.getBytes(StandardCharsets.UTF_8);
        Path dir = Path.of(System.getProperty("java.io.tmpdir"), "snout-engine");
        Files.createDirectories(dir);
        Path script = dir.resolve(prefix + "-" + sha256(bytes).substring(0, 16) + ".gradle");
        if (!Files.exists(script)) {
            Path temp = Files.createTempFile(dir, prefix, ".tmp");
            Files.write(temp, bytes);
            Files.move(temp, script, StandardCopyOption.REPLACE_EXISTING, StandardCopyOption.ATOMIC_MOVE);
        }
        return script;
    }

    static String sha256(byte[] bytes) {
        try {
            return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes));
        } catch (NoSuchAlgorithmException e) {
            throw new IllegalStateException(e);
        }
    }

    static String groovyString(String s) {
        return "'" + s.replace("\\", "\\\\").replace("'", "\\'") + "'";
    }
}
