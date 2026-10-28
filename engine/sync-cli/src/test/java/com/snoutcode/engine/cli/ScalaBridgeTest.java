package com.snoutcode.engine.cli;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

/** How the Scala bridge plans a model: the shared folders it compiles once, and for whom. */
class ScalaBridgeTest {
    @TempDir
    Path dir;

    private Path buildRoot;
    private Path bridgeRoot;
    private Path shared;

    private void layout() throws IOException {
        buildRoot = dir.resolve("cache/build/h");
        bridgeRoot = dir.resolve("cache/bridge/h");
        shared = dir.resolve("core/src/main/java");
        Files.createDirectories(shared.resolve("demo"));
        Files.writeString(shared.resolve("demo/Tone.scala"), "package demo\nobject Tone\n");
        for (String b : List.of("alpha", "beta", "gamma")) {
            Files.createDirectories(dir.resolve(b + "/src/main/scala/demo"));
            Files.writeString(dir.resolve(b + "/src/main/scala/demo/" + b + ".scala"), "package demo\nobject X\n");
        }
    }

    private static JsonObject module(String group, String name, String version, Path file) throws IOException {
        Files.createDirectories(file.getParent());
        if (!Files.exists(file)) {
            Files.writeString(file, "jar");
        }
        JsonObject e = new JsonObject();
        e.addProperty("kind", "module");
        e.addProperty("group", group);
        e.addProperty("name", name);
        e.addProperty("version", version);
        e.addProperty("file", file.toString());
        return e;
    }

    /** One Gradle build's entry: a root project whose main Scala source set lists its own folder and the shared one. */
    private JsonObject build(String id, String release, String scala, JsonObject... extra) throws IOException {
        Path root = dir.resolve(id);
        JsonObject sourceSet = new JsonObject();
        sourceSet.addProperty("name", "main");
        JsonObject scalaSources = new JsonObject();
        JsonArray srcDirs = new JsonArray();
        srcDirs.add(root.resolve("src/main/scala").toString());
        srcDirs.add(shared.toString());
        scalaSources.add("srcDirs", srcDirs);
        scalaSources.addProperty("compileTask", ":compileScala");
        scalaSources.addProperty("outputDir", buildRoot.resolve(id + "-0123/classes/scala/main").toString());
        JsonObject sources = new JsonObject();
        sources.add("scala", scalaSources);
        sourceSet.add("sources", sources);
        JsonObject compile = new JsonObject();
        JsonObject scalaCompile = new JsonObject();
        scalaCompile.addProperty("release", release);
        compile.add("scala", scalaCompile);
        sourceSet.add("compile", compile);
        JsonArray classpath = new JsonArray();
        classpath.add(module("org.scala-lang", "scala-library", scala, dir.resolve("jars/scala-library-" + scala + ".jar")));
        for (JsonObject e : extra) {
            classpath.add(e);
        }
        sourceSet.add("compileClasspath", classpath);
        sourceSet.add("nullable", null);
        JsonObject project = new JsonObject();
        project.addProperty("path", ":");
        project.addProperty("name", id);
        project.addProperty("projectDir", root.toString());
        JsonArray sourceSets = new JsonArray();
        sourceSets.add(sourceSet);
        project.add("sourceSets", sourceSets);
        JsonObject b = new JsonObject();
        b.addProperty("name", id);
        b.addProperty("kind", "root");
        b.addProperty("rootDir", root.toString());
        JsonArray projects = new JsonArray();
        projects.add(project);
        b.add("projects", projects);
        JsonObject entry = new JsonObject();
        entry.addProperty("id", id);
        entry.addProperty("status", "synced");
        entry.addProperty("rootDir", root.toString());
        entry.addProperty("gradleVersion", "8.1.1");
        JsonArray builds = new JsonArray();
        builds.add(b);
        entry.add("builds", builds);
        return entry;
    }

    private ProjectSync.Plan plan(String id) {
        Sync.Options o = new Sync.Options();
        o.projectDir = dir.resolve(id).toFile();
        o.buildRoot = buildRoot.toFile();
        o.bridgeRoot = bridgeRoot.toFile();
        return new ProjectSync.Plan(id, dir.resolve(id).toFile(), "8.1.1", null, o, null);
    }

    private JsonObject model() throws IOException {
        JsonObject model = new JsonObject();
        model.addProperty("rootDir", dir.toString());
        JsonArray builds = new JsonArray();
        builds.add(build("alpha", "17", "2.13.15",
            module("com.example", "lib", "1.0", dir.resolve("jars/lib-1.0.jar"))));
        builds.add(build("beta", "21", "2.13.12",
            module("com.example", "lib", "1.2", dir.resolve("jars/lib-1.2.jar")),
            module("com.example", "extra", "3.0", dir.resolve("jars/extra-3.0.jar"))));
        // Another Scala version: it cannot use the others' classes.
        builds.add(build("gamma", "21", "2.12.18"));
        model.add("gradleBuilds", builds);
        return model;
    }

    @Test
    void compilesASharedFolderOnceForTheBuildsOfOneScalaVersion() throws IOException {
        layout();
        JsonObject model = model();
        ScalaBridge.Work work = ScalaBridge.plan(model, List.of(plan("alpha"), plan("beta"), plan("gamma")), null);
        assertEquals(1, work.units.size(), "one unit, for the 2.13 builds");
        ScalaBridge.Unit unit = work.units.get(0);
        assertEquals(List.of(shared.toAbsolutePath().normalize()), unit.dirs);
        assertEquals(List.of("alpha", "beta"), unit.users.stream().map(c -> c.buildId).toList());
        assertEquals("beta", unit.rep.buildId, "the user with the newest Java");
        assertEquals("core", unit.name);
        // The union of the classpaths: alpha's newer Scala library replaces beta's; beta's lib 1.2 is the newest.
        assertEquals(List.of(dir.resolve("jars/scala-library-2.13.15.jar").toAbsolutePath().normalize().toString()), unit.add);
        assertEquals(List.of(dir.resolve("jars/scala-library-2.13.12.jar").toAbsolutePath().normalize().toString()), unit.drop);

        assertTrue(work.bridged("alpha"));
        assertFalse(work.bridged("gamma"));
        ScalaBridge.Compile alpha = work.compilesOf("alpha").get(0);
        assertTrue(alpha.compiledDir.startsWith(bridgeRoot.toAbsolutePath().normalize()), alpha.compiledDir.toString());
        assertEquals(work.compilesOf("gamma").get(0).outputDir, work.compilesOf("gamma").get(0).compiledDir);

        ScalaBridge.annotate(work);
        // alpha gets its own folder and the unit's, never one of its own for the shared folder.
        List<String> sources = new ArrayList<>();
        for (JsonElement f : alpha.sourceSet.getAsJsonObject("scalaBridge").getAsJsonArray("folders")) {
            sources.add(f.getAsJsonObject().get("sourceDir").getAsString());
        }
        assertEquals(List.of(dir.resolve("alpha/src/main/scala").toAbsolutePath().normalize().toString(),
            shared.toAbsolutePath().normalize().toString()), sources);
        String unitFolder = alpha.sourceSet.getAsJsonObject("scalaBridge").getAsJsonArray("folders").get(1)
            .getAsJsonObject().get("classesDir").getAsString();
        assertTrue(unitFolder.startsWith(unit.home.toString()), unitFolder);
        assertTrue(Files.isDirectory(Path.of(unitFolder)), "created empty, for jdtls to import at once");
    }

    @Test
    void takesNewerAndMissingLibrariesFromTheOtherUsers() throws IOException {
        layout();
        JsonObject model = model();
        // alpha with the newest Java now represents the unit.
        model.getAsJsonArray("gradleBuilds").get(0).getAsJsonObject().getAsJsonArray("builds").get(0).getAsJsonObject()
            .getAsJsonArray("projects").get(0).getAsJsonObject().getAsJsonArray("sourceSets").get(0).getAsJsonObject()
            .getAsJsonObject("compile").getAsJsonObject("scala").addProperty("release", "25");
        ScalaBridge.Work work = ScalaBridge.plan(model, List.of(plan("alpha"), plan("beta"), plan("gamma")), null);
        ScalaBridge.Unit unit = work.units.get(0);
        assertEquals("alpha", unit.rep.buildId);
        assertTrue(unit.add.contains(dir.resolve("jars/lib-1.2.jar").toAbsolutePath().normalize().toString()), unit.add.toString());
        assertTrue(unit.add.contains(dir.resolve("jars/extra-3.0.jar").toAbsolutePath().normalize().toString()), unit.add.toString());
        assertTrue(unit.drop.contains(dir.resolve("jars/lib-1.0.jar").toAbsolutePath().normalize().toString()), unit.drop.toString());
        assertFalse(unit.add.stream().anyMatch(a -> a.contains("scala-library")), "alpha's 2.13.15 is the newest already");
    }

    @Test
    void fingerprintsTheModelAsItsFileHoldsIt() throws IOException {
        layout();
        JsonObject model = model();
        ScalaBridge.Work work = ScalaBridge.plan(model, List.of(plan("alpha"), plan("beta")), null);
        String before = ScalaBridge.fingerprint(work, "alpha");
        // The model file leaves nulls out; read back, the model must fingerprint the same.
        JsonObject read = JsonParser.parseString(new com.google.gson.GsonBuilder().create().toJson(model)).getAsJsonObject();
        ScalaBridge.Work again = ScalaBridge.plan(read, List.of(plan("alpha"), plan("beta")), null);
        assertEquals(before, ScalaBridge.fingerprint(again, "alpha"));
        String unit = ScalaBridge.fingerprint(work.units.get(0));
        assertEquals(unit, ScalaBridge.fingerprint(again.units.get(0)));

        Files.writeString(shared.resolve("demo/Tone.scala"), "package demo\nobject Tone { def x = 1 }\n");
        assertFalse(unit.equals(ScalaBridge.fingerprint(work.units.get(0))), "a changed shared file changes the unit's");
        Files.writeString(dir.resolve("alpha/src/main/scala/demo/alpha.scala"), "package demo\nobject X { def y = 2 }\n");
        assertFalse(before.equals(ScalaBridge.fingerprint(work, "alpha")), "a changed own file changes the build's");
    }

    @Test
    void findsTheUnitsAndBuildsThatHoldAFile() throws IOException {
        layout();
        ScalaBridge.Work work = ScalaBridge.plan(model(), List.of(plan("alpha"), plan("beta"), plan("gamma")), null);
        assertEquals(1, ScalaBridge.unitsHolding(work, List.of(shared.resolve("demo/Tone.scala"))).size());
        assertEquals(List.of("gamma"), new ArrayList<>(ScalaBridge.buildsHolding(work, List.of(shared.resolve("demo/Tone.scala")))),
            "gamma compiles the folder itself");
        assertEquals(List.of("alpha"), new ArrayList<>(ScalaBridge.buildsHolding(work, List.of(dir.resolve("alpha/src/main/scala/demo/alpha.scala")))));
    }
}
