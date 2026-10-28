package com.snoutcode.engine.cli;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;

import java.io.File;
import java.io.IOException;
import java.io.OutputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.List;
import java.util.stream.Stream;
import java.util.zip.ZipEntry;
import java.util.zip.ZipOutputStream;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * Syncs a small composite (two subprojects, an included build, buildSrc, an applied script,
 * an annotation processor and a local jar) with a real Gradle daemon and checks the model.
 */
class SyncIntegrationTest {
    private static Path root;
    private static JsonObject model;

    @BeforeAll
    static void sync() throws IOException {
        Path scratch = Path.of(System.getProperty("snout.test.scratch"));
        root = scratch.resolve("composite-" + System.nanoTime());
        copy(Path.of(System.getProperty("snout.test.fixtures"), "composite"), root);
        Files.createDirectories(root.resolve("libs"));
        emptyJar(root.resolve("libs/local.jar"));
        emptyJar(root.resolve("libs/processor.jar"));
        Files.createDirectories(root.resolve("shared/src/main/java"));

        Sync.Options options = new Sync.Options();
        options.projectDir = root.toFile();
        options.gradleVersion = System.getProperty("snout.test.gradleVersion");
        options.sources = false;
        options.gradleOutput = OutputStream.nullOutputStream();
        model = Sync.run(options);
        Files.writeString(root.resolve("model.json"), model.toString());
    }

    @Test
    void findsRootIncludedAndBuildSrcBuilds() {
        List<String> kinds = new ArrayList<>();
        for (JsonElement b : model.getAsJsonArray("builds")) {
            JsonObject build = b.getAsJsonObject();
            kinds.add(build.get("kind").getAsString() + ":" + build.get("name").getAsString());
            assertEquals(0, build.getAsJsonArray("errors").size(), build.toString());
        }
        assertTrue(kinds.contains("root:demo"), kinds.toString());
        assertTrue(kinds.contains("included:tools"), kinds.toString());
        assertTrue(kinds.contains("buildSrc:buildSrc"), kinds.toString());
        assertEquals(3, build("root").getAsJsonArray("projects").size());
    }

    @Test
    void appProjectHasReferencesAndScripts() throws IOException {
        JsonObject app = project("root", ":app");
        JsonObject main = sourceSet(app, "main");

        List<String> refs = new ArrayList<>();
        for (JsonElement e : main.getAsJsonArray("compileClasspath")) {
            JsonObject entry = e.getAsJsonObject();
            String kind = entry.get("kind").getAsString();
            refs.add(kind.equals("project")
                ? "project " + entry.get("build").getAsString() + " " + entry.get("path").getAsString()
                : kind + " " + new File(entry.get("file").getAsString()).getName());
        }
        assertTrue(refs.contains("project : :core"), refs.toString());
        assertTrue(refs.contains("project :tools :"), refs.toString());
        assertTrue(refs.contains("file local.jar"), refs.toString());

        String shared = root.resolve("gradle/shared-deps.gradle").toFile().getCanonicalPath();
        JsonArray scripts = app.getAsJsonObject("appliedScripts").getAsJsonArray("scripts");
        assertEquals(1, scripts.size(), scripts.toString());
        assertEquals(shared, new File(scripts.get(0).getAsString()).getCanonicalPath());
        assertTrue(contains(project("root", ":").getAsJsonObject("build").getAsJsonArray("resyncTriggers"), shared));

        List<String> javaDirs = strings(main.getAsJsonArray("javaSourceDirs"));
        assertTrue(javaDirs.stream().anyMatch(d -> d.endsWith("shared/src/main/java")), javaDirs.toString());

        JsonObject test = app.getAsJsonArray("testTasks").get(0).getAsJsonObject();
        assertEquals("junit-platform", test.get("framework").getAsString());
    }

    @Test
    void coreProjectHasAnnotationProcessingAndCompilerSettings() {
        JsonObject main = sourceSet(project("root", ":core"), "main");
        List<String> processorPath = strings(main.getAsJsonArray("annotationProcessorPath"));
        assertEquals(1, processorPath.size());
        assertTrue(processorPath.get(0).endsWith("processor.jar"));
        JsonObject generated = main.getAsJsonArray("generatedSources").get(0).getAsJsonObject();
        assertTrue(generated.get("dir").getAsString().endsWith("build/generated/sources/annotationProcessor/java/main"));
        JsonObject javac = main.getAsJsonObject("compile").getAsJsonObject("java");
        assertEquals("11", javac.get("sourceCompatibility").getAsString());
        assertTrue(strings(javac.getAsJsonArray("compilerArgs")).contains("-Xlint:deprecation"));
    }

    @Test
    void syncRunsNoTasks() {
        assertTrue(Files.notExists(root.resolve("app/build")), "app/build exists");
        assertTrue(Files.notExists(root.resolve("core/build")), "core/build exists");
    }

    private static JsonObject build(String kind) {
        for (JsonElement b : model.getAsJsonArray("builds")) {
            if (kind.equals(b.getAsJsonObject().get("kind").getAsString())) {
                return b.getAsJsonObject();
            }
        }
        throw new AssertionError("no " + kind + " build");
    }

    private static JsonObject project(String buildKind, String path) {
        for (JsonElement p : build(buildKind).getAsJsonArray("projects")) {
            if (path.equals(p.getAsJsonObject().get("path").getAsString())) {
                return p.getAsJsonObject();
            }
        }
        throw new AssertionError("no project " + path);
    }

    private static JsonObject sourceSet(JsonObject project, String name) {
        for (JsonElement s : project.getAsJsonArray("sourceSets")) {
            if (name.equals(s.getAsJsonObject().get("name").getAsString())) {
                return s.getAsJsonObject();
            }
        }
        throw new AssertionError("no source set " + name);
    }

    private static List<String> strings(JsonArray array) {
        List<String> out = new ArrayList<>();
        array.forEach(e -> out.add(e.getAsString()));
        return out;
    }

    private static boolean contains(JsonArray array, String canonicalPath) {
        return strings(array).stream().anyMatch(s -> {
            try {
                return new File(s).getCanonicalPath().equals(canonicalPath);
            } catch (IOException e) {
                return false;
            }
        });
    }

    private static void emptyJar(Path jar) throws IOException {
        try (ZipOutputStream zip = new ZipOutputStream(Files.newOutputStream(jar))) {
            zip.putNextEntry(new ZipEntry("META-INF/MANIFEST.MF"));
            zip.write("Manifest-Version: 1.0\n".getBytes());
            zip.closeEntry();
        }
    }

    static void copy(Path from, Path to) throws IOException {
        try (Stream<Path> files = Files.walk(from)) {
            for (Path source : (Iterable<Path>) files::iterator) {
                Path target = to.resolve(from.relativize(source).toString());
                if (Files.isDirectory(source)) {
                    Files.createDirectories(target);
                } else {
                    Files.copy(source, target, StandardCopyOption.REPLACE_EXISTING);
                }
            }
        }
    }
}
