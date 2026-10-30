package com.snoutcode.engine.cli;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.io.File;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Set;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/** When a start may take the last sync's model, and which builds it syncs again anyway. */
class ModelCacheTest {
    @TempDir
    Path dir;

    private JsonObject model(Path applied) {
        JsonObject model = new JsonObject();
        JsonArray scripts = new JsonArray();
        JsonObject build = new JsonObject();
        build.addProperty("build", "app");
        JsonArray files = new JsonArray();
        JsonObject file = new JsonObject();
        file.addProperty("path", applied.toString());
        file.addProperty("kind", "applied");
        files.add(file);
        build.add("files", files);
        scripts.add(build);
        model.add("gradleScripts", scripts);
        model.add("gradleBuilds", new JsonArray());
        return model;
    }

    private ProjectSync.Plan plan(String javaHome) {
        Sync.Options o = new Sync.Options();
        o.javaHome = javaHome == null ? null : new File(javaHome);
        return new ProjectSync.Plan("app", dir.resolve("app").toFile(), "8.10.2", null, o, null);
    }

    @Test
    void keepsTheModelWhileItsInputsHold() throws IOException {
        Path app = dir.resolve("app");
        Files.createDirectories(app);
        Files.writeString(app.resolve("build.gradle"), "plugins { id 'java' }\n");
        Path applied = dir.resolve("deps.gradle");
        Files.writeString(applied, "dependencies {}\n");
        Path userHome = dir.resolve("gradle-home");
        List<ProjectSync.Plan> plans = List.of(plan("/jdk17"));
        JsonObject inputs = ModelCache.inputs(model(applied), plans, "key", userHome.toFile());
        assertNull(ModelCache.changed(inputs, plans, "key"));

        assertNotNull(ModelCache.changed(inputs, plans, "other key"), "other options");
        assertNotNull(ModelCache.changed(inputs, List.of(plan("/jdk21")), "key"), "another daemon JDK");
        assertNotNull(ModelCache.changed(null, plans, "key"), "no record");

        Files.writeString(applied, "dependencies { implementation 'a:b:1' }\n");
        assertTrue(ModelCache.changed(inputs, plans, "key").contains("deps.gradle"));
        Files.writeString(applied, "dependencies {}\n");
        assertNull(ModelCache.changed(inputs, plans, "key"), "by content, not time");

        Files.writeString(app.resolve("gradle.properties"), "org.gradle.jvmargs=-Xmx2g\n");
        assertTrue(ModelCache.changed(inputs, plans, "key").contains("gradle.properties"), "a new properties file");
        Files.delete(app.resolve("gradle.properties"));

        Files.createDirectories(app.resolve("buildSrc/src/main/groovy"));
        Files.writeString(app.resolve("buildSrc/src/main/groovy/Conv.groovy"), "class Conv {}\n");
        assertTrue(ModelCache.changed(inputs, plans, "key").contains("buildSrc"), "buildSrc's sources");
    }

    @Test
    void syncsAgainWhatASyncMayGiveDifferently() {
        JsonObject model = new JsonObject();
        JsonArray builds = new JsonArray();
        builds.add(entry("offline", "failed", "Could not GET 'https://repo.maven.apache.org/x.pom'. Connection refused", null));
        builds.add(entry("broken", "failed", "Build file 'build.gradle' line: 3: unexpected token", null));
        builds.add(entry("nojdk", "failed", "Needs JDK 17", 17));
        JsonObject snapshot = entry("snapshot", "synced", null, null);
        JsonObject sourceSet = new JsonObject();
        JsonArray cp = new JsonArray();
        JsonObject lib = new JsonObject();
        lib.addProperty("kind", "module");
        lib.addProperty("version", "1.0-SNAPSHOT");
        cp.add(lib);
        sourceSet.add("compileClasspath", cp);
        JsonArray sourceSets = new JsonArray();
        sourceSets.add(sourceSet);
        JsonObject project = new JsonObject();
        project.add("sourceSets", sourceSets);
        JsonArray projects = new JsonArray();
        projects.add(project);
        JsonObject b = new JsonObject();
        b.add("projects", projects);
        JsonArray bs = new JsonArray();
        bs.add(b);
        snapshot.add("builds", bs);
        builds.add(snapshot);
        // A build the start went on without, whose sync never came in (the engine ended first).
        JsonObject late = entry("late", "syncing", null, null);
        late.addProperty("late", true);
        builds.add(late);
        model.add("gradleBuilds", builds);
        assertEquals(Set.of("offline", "snapshot", "late"), ModelCache.uncertain(model));
    }

    private static JsonObject entry(String id, String status, String error, Integer needsJdk) {
        JsonObject e = new JsonObject();
        e.addProperty("id", id);
        e.addProperty("status", status);
        if (error != null) {
            e.addProperty("error", error);
        }
        if (needsJdk != null) {
            e.addProperty("needsJdk", needsJdk);
        }
        e.add("builds", new JsonArray());
        return e;
    }
}
