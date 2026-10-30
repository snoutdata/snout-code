package com.snoutcode.engine.cli;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;

import java.io.File;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * Syncs a Maven reactor (a parent that aggregates, a BOM import, profiles, a library with a test
 * jar and an application with every scope and an annotation processor path) with Maven's own
 * model builder and resolver, against the local repository the build names (never ~/.m2), and
 * checks the model; and a folder holding a Gradle build and a Maven build side by side.
 */
class MavenSyncTest {
    private static Path root;
    private static JsonObject model;

    @BeforeAll
    static void sync() throws IOException {
        root = Path.of(System.getProperty("snout.test.scratch")).resolve("maven-" + System.nanoTime()).resolve("maven-multi");
        SyncIntegrationTest.copy(Path.of(System.getProperty("snout.test.fixtures"), "maven-multi"), root);
        model = ProjectSync.run(options(root, List.of()));
        Files.writeString(root.resolve("../model.json"), model.toString());
    }

    static ProjectSync.Options options(Path folder, List<String> profiles) {
        ProjectSync.Options options = new ProjectSync.Options();
        options.sync.projectDir = folder.toFile();
        options.sync.sources = false;
        options.mavenProfiles.addAll(profiles);
        return options;
    }

    @Test
    void readsTheReactorAsOneBuildOfModules() {
        JsonObject build = model.getAsJsonArray("gradleBuilds").get(0).getAsJsonObject();
        assertEquals("maven", build.get("tool").getAsString());
        assertEquals("synced", build.get("status").getAsString(), build.toString());
        List<String> ids = new ArrayList<>();
        for (JsonElement m : model.getAsJsonArray("modules")) {
            ids.add(m.getAsJsonObject().get("id").getAsString());
            assertEquals("maven", m.getAsJsonObject().get("tool").getAsString());
        }
        assertEquals(List.of("maven-multi", "maven-multi:core", "maven-multi:app"), ids);
        assertTrue(model.getAsJsonArray("failedBuilds").isEmpty(), model.get("failedBuilds").toString());
        // The POMs are the build's scripts: a change to one asks for a sync.
        String scripts = model.getAsJsonArray("gradleScripts").toString();
        assertTrue(scripts.contains("app/pom.xml") && scripts.contains("core/pom.xml"), scripts);
    }

    @Test
    void scopesBecomeClasspaths() {
        JsonObject deps = module("maven-multi:app").getAsJsonObject("dependencies");
        assertTrue(ids(deps, "compile").contains("maven-multi:core"), deps.toString());
        assertTrue(ids(deps, "compile").contains("org.apache.commons:commons-lang3:3.17.0"), "managed in the parent: " + deps);
        assertEquals(List.of("jakarta.annotation:jakarta.annotation-api:2.1.1"), ids(deps, "provided"));
        assertTrue(ids(deps, "runtime").contains("org.slf4j:slf4j-simple:2.0.16"), deps.toString());
        assertTrue(ids(deps, "test").contains("org.junit.jupiter:junit-jupiter-api:5.11.3"), "from the imported BOM: " + deps);
        for (JsonElement l : model.getAsJsonArray("libraries")) {
            String file = l.getAsJsonObject().get("file").getAsString();
            assertTrue(file.startsWith(System.getProperty("snout.test.m2")), "resolved into the test's local repository: " + file);
        }
    }

    @Test
    void theTestJarIsTheLibrarysTestClasses() {
        JsonObject test = sourceSet(project(":app"), "test");
        boolean found = false;
        for (JsonElement e : test.getAsJsonArray("compileClasspath")) {
            JsonObject entry = e.getAsJsonObject();
            if ("project".equals(entry.get("kind").getAsString()) && entry.toString().contains("core/target/test-classes")) {
                found = true;
            }
        }
        assertTrue(found, test.get("compileClasspath").toString());
    }

    @Test
    void javaLevelsParametersAndProcessors() {
        JsonObject main = sourceSet(project(":app"), "main");
        JsonObject javac = main.getAsJsonObject("compile").getAsJsonObject("java");
        assertEquals("17", javac.get("release").getAsString());
        assertEquals("UTF-8", javac.get("encoding").getAsString());
        assertTrue(javac.getAsJsonArray("compilerArgs").toString().contains("-parameters"), javac.toString());
        assertTrue(main.getAsJsonArray("annotationProcessorPath").toString().contains("auto-service-1.1.1.jar"), main.toString());
        assertEquals("17", module("maven-multi:app").getAsJsonObject("jdk").get("languageLevel").getAsString());
        String roots = module("maven-multi:app").getAsJsonArray("sourceRoots").toString();
        assertTrue(roots.contains("target/generated-sources/annotations"), roots);
        assertTrue(module("maven-multi:app").getAsJsonArray("excludedDirs").toString().contains("app/target"));
    }

    @Test
    void profilesAndThePluginsTheWindowShows() throws IOException {
        JsonObject maven = project(":app").getAsJsonObject("maven");
        String profiles = maven.getAsJsonArray("profiles").toString();
        assertTrue(profiles.contains("{\"id\":\"fast\",\"active\":true"), profiles);
        assertTrue(profiles.contains("{\"id\":\"extra\",\"active\":false"), profiles);
        assertFalse(profiles.contains("release-profile"), "the super POM's are left out: " + profiles);
        assertEquals("demo", maven.get("groupId").getAsString());
        // Turning a profile on brings its module in.
        JsonObject withExtra = ProjectSync.run(options(root, List.of("extra")));
        List<String> ids = new ArrayList<>();
        withExtra.getAsJsonArray("modules").forEach(m -> ids.add(m.getAsJsonObject().get("id").getAsString()));
        assertTrue(ids.contains("maven-multi:extra"), ids.toString());
    }

    @Test
    void aFolderWithBothBuildToolsOpensAsOneProject() throws IOException {
        Path folder = root.getParent().resolveSibling("mixed-" + System.nanoTime());
        Files.createDirectories(folder.resolve("gradle-app"));
        Files.writeString(folder.resolve("gradle-app/settings.gradle"), "rootProject.name = 'gradle-app'\n");
        SyncIntegrationTest.copy(root.resolve("core"), folder.resolve("lib"));
        Files.writeString(folder.resolve("lib/pom.xml"), Files.readString(folder.resolve("lib/pom.xml"))
            .replaceAll("(?s)<parent>.*?</parent>", "<groupId>demo</groupId><version>1.0</version>")
            .replace("<scope>test</scope>", "<version>5.11.3</version><scope>test</scope>")
            .replace("<artifactId>commons-lang3</artifactId>", "<artifactId>commons-lang3</artifactId><version>3.17.0</version>"));
        List<File> builds = Discovery.builds(folder.toFile());
        assertEquals(2, builds.size(), builds.toString());
        assertTrue(Discovery.isMaven(folder.resolve("lib").toFile()));
        assertFalse(Discovery.isMaven(folder.resolve("gradle-app").toFile()));
        List<ProjectSync.Plan> plans = ProjectSync.plan(options(folder, List.of()));
        assertTrue(plans.stream().anyMatch(ProjectSync.Plan::maven));
        assertTrue(plans.stream().anyMatch(p -> !p.maven()));
    }

    @Test
    void aFolderInsideAReactorIsTheReactor() throws IOException {
        assertEquals(List.of(root.toFile().getCanonicalFile()), Discovery.builds(root.resolve("app/src/main").toFile()));
    }

    private static List<String> ids(JsonObject deps, String scope) {
        List<String> out = new ArrayList<>();
        deps.getAsJsonArray(scope).forEach(d -> out.add(d.getAsJsonObject().get("id").getAsString()));
        return out;
    }

    private static JsonObject module(String id) {
        for (JsonElement m : model.getAsJsonArray("modules")) {
            if (id.equals(m.getAsJsonObject().get("id").getAsString())) {
                return m.getAsJsonObject();
            }
        }
        throw new AssertionError("no module " + id);
    }

    static JsonObject project(String path) {
        JsonArray projects = model.getAsJsonArray("gradleBuilds").get(0).getAsJsonObject().getAsJsonArray("builds").get(0)
            .getAsJsonObject().getAsJsonArray("projects");
        for (JsonElement p : projects) {
            if (path.equals(p.getAsJsonObject().get("path").getAsString())) {
                return p.getAsJsonObject();
            }
        }
        throw new AssertionError("no project " + path);
    }

    static JsonObject sourceSet(JsonObject project, String name) {
        for (JsonElement s : project.getAsJsonArray("sourceSets")) {
            if (name.equals(s.getAsJsonObject().get("name").getAsString())) {
                return s.getAsJsonObject();
            }
        }
        assertNotNull(null, "no source set " + name);
        return null;
    }
}
