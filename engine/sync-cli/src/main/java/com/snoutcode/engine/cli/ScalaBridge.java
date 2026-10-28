package com.snoutcode.engine.cli;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import org.gradle.tooling.BuildException;
import org.gradle.tooling.BuildLauncher;
import org.gradle.tooling.ProjectConnection;

import java.io.File;
import java.io.IOException;
import java.io.OutputStream;
import java.io.UncheckedIOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.stream.Stream;

/**
 * Puts compiled Scala classes where jdtls can see them. jdtls cannot read Scala source, so Java
 * code that uses Scala types needs their class files on its classpath.
 *
 * <p>{@link #compile} runs Gradle's Scala compile task for every source set that has Scala
 * files, and nothing else, with build directories redirected out of the repository (see
 * {@link Sync#buildDirInitScript}). It then records each source set's Scala classes in the model
 * under {@code scalaBridge.folders}; the jdtls importer adds each folder as a library, with its
 * Scala source root attached. The folders hold only the classes compiled from Scala (see
 * {@link ScalaClassFolder}): jdtls has the Java ones as sources.
 *
 * <p>This is the one entry point: recompiling when Scala files change can call it again.
 */
public final class ScalaBridge {
    /** The outcome of one {@link #compile}. */
    public static final class Result {
        public final List<String> tasks = new ArrayList<>();
        public int sourceSets;
        public long millis;
        public String error;
    }

    private ScalaBridge() {
    }

    /**
     * Compiles the Scala source sets of a model that was synced with {@code options.buildRoot}
     * set, and annotates the model. A failed compile still annotates the class folders that exist:
     * classes from an earlier compile are better than none.
     */
    public static Result compile(JsonObject model, Sync.Options options) throws IOException {
        long start = System.nanoTime();
        Result result = new Result();
        Map<String, JsonObject> classesDirs = new LinkedHashMap<>();
        for (JsonElement b : model.getAsJsonArray("builds")) {
            JsonObject build = b.getAsJsonObject();
            String kind = string(build, "kind");
            if (!"root".equals(kind) && !"included".equals(kind)) {
                continue; // buildSrc code is not called from the project's sources
            }
            String prefix = "root".equals(kind) ? "" : ":" + string(build, "name");
            for (JsonElement p : build.getAsJsonArray("projects")) {
                JsonObject project = p.getAsJsonObject();
                if (!project.has("sourceSets")) {
                    continue;
                }
                for (JsonElement s : project.getAsJsonArray("sourceSets")) {
                    JsonObject sourceSet = s.getAsJsonObject();
                    JsonObject scala = object(object(sourceSet, "sources"), "scala");
                    String task = string(scala, "compileTask");
                    String outputDir = string(scala, "outputDir");
                    if (task == null || outputDir == null || !hasScalaFiles(scala)) {
                        continue;
                    }
                    result.tasks.add(prefix + task);
                    classesDirs.put(outputDir, sourceSet);
                }
            }
        }
        if (!result.tasks.isEmpty()) {
            File root = new File(model.get("rootDir").getAsString());
            List<Path> initScripts = new ArrayList<>();
            if (options.buildRoot != null) {
                initScripts.add(Sync.buildDirInitScript(options.buildRoot));
            }
            try (ProjectConnection connection = Sync.connector(root, options).connect()) {
                List<String> args = Sync.arguments(initScripts, options.projectCacheDir, options);
                args.add("--continue");
                BuildLauncher launcher = connection.newBuild().forTasks(result.tasks.toArray(new String[0])).withArguments(args);
                if (options.javaHome != null) {
                    launcher.setJavaHome(options.javaHome);
                }
                OutputStream out = options.gradleOutput != null ? options.gradleOutput : OutputStream.nullOutputStream();
                launcher.setStandardOutput(out);
                launcher.setStandardError(out);
                launcher.run();
            } catch (BuildException e) {
                result.error = SnoutEngine.describe(e);
            }
        }
        for (Map.Entry<String, JsonObject> e : classesDirs.entrySet()) {
            Path compiled = Path.of(e.getKey());
            if (!Files.isDirectory(compiled)) {
                continue;
            }
            JsonObject scala = object(object(e.getValue(), "sources"), "scala");
            List<Path> roots = new ArrayList<>();
            for (JsonElement d : scala.getAsJsonArray("srcDirs")) {
                roots.add(Path.of(d.getAsString()));
            }
            Path mirrors = compiled.resolveSibling(compiled.getFileName() + "-scala");
            JsonObject bridge = new JsonObject();
            bridge.addProperty("compiledDir", compiled.toString());
            JsonArray folders = new JsonArray();
            for (ScalaClassFolder.Folder folder : ScalaClassFolder.mirror(compiled, mirrors, roots)) {
                JsonObject f = new JsonObject();
                f.addProperty("classesDir", folder.classesDir().toString());
                f.addProperty("sourceDir", folder.sourceDir() == null ? null : folder.sourceDir().toString());
                f.addProperty("classes", folder.classes());
                folders.add(f);
            }
            bridge.add("folders", folders);
            e.getValue().add("scalaBridge", bridge);
            result.sourceSets++;
        }
        result.millis = (System.nanoTime() - start) / 1_000_000;
        return result;
    }

    private static boolean hasScalaFiles(JsonObject scala) {
        if (scala == null || !scala.has("srcDirs")) {
            return false;
        }
        for (JsonElement d : scala.getAsJsonArray("srcDirs")) {
            Path dir = Path.of(d.getAsString());
            if (!Files.isDirectory(dir)) {
                continue;
            }
            try (Stream<Path> files = Files.walk(dir)) {
                if (files.anyMatch(f -> f.getFileName().toString().endsWith(".scala"))) {
                    return true;
                }
            } catch (IOException e) {
                throw new UncheckedIOException(e);
            }
        }
        return false;
    }

    private static String string(JsonObject o, String key) {
        return o == null || !o.has(key) || o.get(key).isJsonNull() ? null : o.get(key).getAsString();
    }

    private static JsonObject object(JsonObject o, String key) {
        return o == null || !o.has(key) || !o.get(key).isJsonObject() ? null : o.getAsJsonObject(key);
    }
}
