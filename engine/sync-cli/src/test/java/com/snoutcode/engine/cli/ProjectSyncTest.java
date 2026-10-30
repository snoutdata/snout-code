package com.snoutcode.engine.cli;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.io.File;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/** Discovery, JDK choice, failure reasons and the project view, without Gradle. */
class ProjectSyncTest {
    @TempDir
    Path dir;

    private void touch(String... files) throws IOException {
        for (String f : files) {
            Path p = dir.resolve(f);
            Files.createDirectories(p.getParent());
            Files.writeString(p, "");
        }
    }

    private List<String> found(Path folder) throws IOException {
        List<String> out = new ArrayList<>();
        for (File f : Discovery.builds(folder.toFile())) {
            Path p = f.toPath();
            Path base = dir.toRealPath();
            out.add(p.equals(base) ? "." : base.relativize(p).toString().replace(File.separatorChar, '/'));
        }
        return out;
    }

    @Test
    void findsEveryBuildUnderAFolderAndSkipsWhatIsNotOne() throws IOException {
        touch("a/settings.gradle", "a/build.gradle", "a/sub/build.gradle", "a/included/settings.gradle",
            "b/build.gradle.kts",
            "group/c/settings.gradle.kts",
            "shared/src/main/java/X.java",
            "node_modules/pkg/build.gradle", ".hidden/settings.gradle", "build/settings.gradle",
            "d/.gradle/settings.gradle", "d/build.gradle");
        assertEquals(List.of("a", "b", "d", "group/c"), found(dir));
    }

    @Test
    void aFolderInsideABuildIsThatBuild() throws IOException {
        touch("a/settings.gradle", "a/app/build.gradle", "a/app/src/main/java/X.java", "lone/build.gradle", "lone/src/X.java");
        assertEquals(List.of("a"), found(dir.resolve("a/app/src")));
        assertEquals(List.of("a"), found(dir.resolve("a")));
        assertEquals(List.of("lone"), found(dir.resolve("lone/src")));
    }

    @Test
    void aFolderInABuildsOutputsIsNotPartOfIt() throws IOException {
        touch("outer/settings.gradle", "outer/build/scratch/one/settings.gradle", "outer/build/scratch/two/build.gradle");
        assertEquals(List.of("outer/build/scratch/one", "outer/build/scratch/two"), found(dir.resolve("outer/build/scratch")));
    }

    @Test
    void choosesTheDaemonJdkFromTheGradleVersion() {
        JavaHomes.Jdk j11 = new JavaHomes.Jdk(new File("/jdk11"), 11, "11.0.20");
        JavaHomes.Jdk j17 = new JavaHomes.Jdk(new File("/jdk17"), 17, "17.0.9");
        JavaHomes.Jdk j21 = new JavaHomes.Jdk(new File("/jdk21"), 21, "21.0.2");
        List<JavaHomes.Jdk> all = List.of(j21, j11, j17);
        assertEquals(j11, JavaHomes.choose("5.2.1", null, j17, all), "Gradle 5 cannot run on 17");
        assertEquals(j11, JavaHomes.choose("6.0.1", 17, j17, all), "Gradle before 6.7 gets 11");
        assertEquals(j17, JavaHomes.choose("8.1.1", 17, null, all));
        assertEquals(j17, JavaHomes.choose("8.1.1", null, j17, all), "the preferred JDK, when Gradle runs on it");
        assertEquals(j17, JavaHomes.choose("8.1.1", null, j21, all), "Gradle 8.1 does not run on 21");
        assertEquals(j21, JavaHomes.choose("8.10.2", 21, j17, all), "a toolchain of 21 beats the preferred 17");
        assertEquals(j11, JavaHomes.choose("8.10.2", null, null, List.of(j11)), "the closest one installed");
        assertNull(JavaHomes.choose("5.2.1", null, null, List.of(j17, j21)));
        JavaHomes.Jdk j8 = new JavaHomes.Jdk(new File("/jdk8"), 8, "1.8.0_402");
        assertNull(JavaHomes.choose("9.1.0", null, j8, List.of(j8)), "Gradle 9 does not run on 8");
    }

    @Test
    void followsGradlesCompatibilityMatrix() {
        assertEquals(21, JavaHomes.maxJava("8.5"));
        assertEquals(20, JavaHomes.maxJava("8.4"));
        assertEquals(19, JavaHomes.maxJava("8.1.1"));
        assertEquals(19, JavaHomes.maxJava("8.0"));
        assertEquals(17, JavaHomes.maxJava("7.3"));
        assertEquals(16, JavaHomes.maxJava("7.0.2"));
        assertEquals(15, JavaHomes.maxJava("6.7"));
        assertEquals(11, JavaHomes.maxJava("5.2.1"));
        assertEquals(8, JavaHomes.maxJava("4.10"));
        assertEquals(17, JavaHomes.minJava("9.0"));

        // Only Java 21 installed: every Gradle before 8.5 needs another JDK, the newest LTS it runs on.
        JavaHomes.Jdk j21 = new JavaHomes.Jdk(new File("/jdk21"), 21, "21.0.2");
        List<JavaHomes.Jdk> only21 = List.of(j21);
        String[][] cases = {{"8.1.1", "17"}, {"8.0", "17"}, {"7.3", "17"}, {"7.0.2", "11"}, {"6.7", "11"}, {"6.0.1", "11"},
            {"5.2.1", "11"}, {"4.10", "8"}};
        for (String[] c : cases) {
            assertNull(JavaHomes.choose(c[0], null, j21, only21), "Gradle " + c[0] + " does not run on 21");
            assertEquals(Integer.valueOf(c[1]), JavaHomes.needs(c[0], null, JavaHomes.choose(c[0], null, j21, only21)), "Gradle " + c[0]);
        }
        assertEquals(j21, JavaHomes.choose("8.5", null, j21, only21));
        assertEquals(17, JavaHomes.needs("8.1.1", 17, null), "a toolchain of 17");
        assertEquals(17, JavaHomes.needs("8.1.1", 21, null), "a toolchain of 21 needs no daemon of 21");
    }

    @Test
    void neverRunsABuildOnAJdkItsGradleCannotRunOn() throws IOException {
        touch("old/settings.gradle", "old/gradle/wrapper/gradle-wrapper.properties");
        Files.writeString(dir.resolve("old/gradle/wrapper/gradle-wrapper.properties"),
            "distributionUrl=https\\://services.gradle.org/distributions/gradle-8.1.1-bin.zip\n");
        ProjectSync.Options options = new ProjectSync.Options();
        options.sync.projectDir = dir.resolve("old").toFile();
        options.sync.javaHome = new File(System.getProperty("java.home"));
        options.jdks.clear();
        List<ProjectSync.Plan> plans = ProjectSync.plan(options);
        ProjectSync.Plan plan = plans.get(0);
        if (plan.jdk() == null) {
            assertEquals(17, plan.needsJdk());
            assertNull(plan.options().javaHome, "no fallback to the preferred JDK Gradle 8.1.1 cannot run on");
        } else {
            assertTrue(plan.jdk().feature() <= 19, plan.jdk().toString());
        }
    }

    @Test
    void aGroovyFailureOnANewJdkNeedsAnotherJdk() {
        JavaHomes.Jdk j21 = new JavaHomes.Jdk(new File("/jdk21"), 21, "21.0.2");
        assertEquals(11, ProjectSync.jdkMismatch("org.codehaus.groovy.GroovyBugError: BUG! exception in phase 'semantic analysis'",
            "6.0.1", j21));
        assertEquals(17, ProjectSync.jdkMismatch("Unsupported class file major version 65", "8.1.1", j21));
        assertNull(ProjectSync.jdkMismatch("Could not find org.example:missing:1.0.", "8.10.2", j21));
        assertEquals("Gradle 6.0.1 does not run on Java 21",
            ProjectSync.reason("java.lang.NoClassDefFoundError: Could not initialize class org.codehaus.groovy.vmplugin.v7.Java7", "6.0.1", j21));
    }

    @Test
    void saysWhichJdkABuildNeeds() {
        JavaHomes.Jdk j17 = new JavaHomes.Jdk(new File("/jdk17"), 17, "17.0.9");
        JavaHomes.Jdk j21 = new JavaHomes.Jdk(new File("/jdk21"), 21, "21.0.2");
        List<JavaHomes.Jdk> installed = List.of(j17, j21);
        assertEquals(11, JavaHomes.needs("5.2.1", null, JavaHomes.choose("5.2.1", null, null, installed)));
        assertEquals(11, JavaHomes.needs("6.0.1", 8, JavaHomes.choose("6.0.1", 8, null, installed)));
        assertNull(JavaHomes.needs("8.1.1", null, JavaHomes.choose("8.1.1", null, null, installed)));
        assertNull(JavaHomes.needs(null, null, null), "no wrapper: the Tooling API picks");
    }

    @Test
    void syncsNothingWithoutAJdkThatRunsTheBuild() {
        ProjectSync.Plan plan = new ProjectSync.Plan("old", dir.toFile(), "5.2.1", null, new Sync.Options(), 11);
        com.google.gson.JsonObject entry = ProjectSync.syncOne(plan, null);
        assertEquals("failed", entry.get("status").getAsString());
        assertEquals(11, entry.get("needsJdk").getAsInt());
        assertEquals("Needs JDK 11, which isn't installed", entry.get("reason").getAsString());
    }

    @Test
    void searchesSnoutCodesJdkFolders() {
        Path home = Path.of("/home/me");
        List<Path> folders = JavaHomes.snoutJdkFolders(home, Path.of("/elsewhere"));
        assertTrue(folders.contains(Lsp.defaultCacheDir().resolve("build-jdk")), folders.toString());
        assertTrue(folders.contains(Path.of("/elsewhere/jdks")), folders.toString());
        if (System.getProperty("os.name").toLowerCase(java.util.Locale.ROOT).contains("mac")) {
            assertTrue(folders.contains(home.resolve("Library/Application Support/SnoutCode/jdks")), folders.toString());
        }
    }

    @Test
    void readsTheWrapperVersionAndTheScriptLevel() throws IOException {
        Path build = dir.resolve("b");
        Files.createDirectories(build.resolve("gradle/wrapper"));
        Files.writeString(build.resolve("gradle/wrapper/gradle-wrapper.properties"),
            "distributionUrl=https\\://services.gradle.org/distributions/gradle-6.0.1-all.zip\n");
        Files.writeString(build.resolve("build.gradle"), """
            // sourceCompatibility = '21' in a comment does not count
            java {
                sourceCompatibility = '11'
                toolchain { languageVersion = JavaLanguageVersion.of(17) }
            }
            """);
        assertEquals("6.0.1", JavaHomes.wrapperVersion(build.toFile()));
        assertEquals(17, JavaHomes.scriptLevel(build.toFile()));
        assertNull(JavaHomes.wrapperVersion(dir.toFile()));
    }

    @Test
    void explainsFailuresInAFewWords() {
        JavaHomes.Jdk j11 = new JavaHomes.Jdk(new File("/jdk11"), 11, "11.0.20");
        assertEquals("build.gradle:5: This build is broken on purpose.", ProjectSync.reason("""
            Could not run build action using connection to Gradle distribution 'x'.
              caused by: A problem occurred evaluating root project 'broken'.
              caused by: Build file '/w/broken/build.gradle' line: 5
            A problem occurred evaluating root project 'broken'.
              caused by: This build is broken on purpose.""", "8.10.2", j11));
        assertEquals("no variant of the build plugin org.springframework.boot:spring-boot-gradle-plugin:3.1.1 fits Gradle 6.0.1"
            + " on Java 11; it needs a newer Gradle or Java", ProjectSync.reason("""
            Could not resolve all artifacts for configuration ':classpath'.
              caused by: Cannot choose between the following variants of org.springframework.boot:spring-boot-gradle-plugin:3.1.1:
              - javadocElements""", "6.0.1", j11));
        // A failure the Tooling API could not deserialize: what Gradle printed tells.
        String printed = """
            FAILURE: Build failed with an exception.

            * What went wrong:
            A problem occurred configuring root project 'old'.
            > Could not resolve all artifacts for configuration ':classpath'.
               > Could not resolve com.example:tool:1.0.
                  > Something deep went wrong.

            * Try:
            Run with --stacktrace option to get the stack trace.
            """;
        String said = GradleSaid.whatWentWrong(printed);
        assertEquals("Something deep went wrong.", GradleSaid.deepest(said));
        String error = SnoutEngine.describe(new GradleSaid(said, new RuntimeException("invalid type code: 00")));
        assertEquals("Something deep went wrong.", ProjectSync.reason(error, "6.0.1", j11));
        assertNull(GradleSaid.whatWentWrong("BUILD SUCCESSFUL"));
        assertEquals("plugin com.example.missing 1.0 was not found",
            ProjectSync.reason("Plugin [id: 'com.example.missing', version: '1.0'] was not found in any of the following sources:",
                "8.1.1", j11));
    }

    @Test
    void aPluginThatNeedsANewerJavaAsksForIt() {
        JavaHomes.Jdk j8 = new JavaHomes.Jdk(new File("/jdk8"), 8, "1.8.0_512");
        String error = "Android Gradle plugin requires Java 11 to run. You are currently using Java 1.8.";
        assertEquals(11, ProjectSync.jdkMismatch(error, "7.0.2", j8));
        JavaHomes.Jdk j11 = new JavaHomes.Jdk(new File("/jdk11"), 11, "11.0.20");
        assertNull(ProjectSync.jdkMismatch(error, "7.0.2", j11), "the JDK it has is new enough: something else is wrong");
    }

    @Test
    void goesOnWithoutTheLastFewBuilds() {
        assertFalse(ProjectSync.goOn(0, 21, 5000, 2000), "every build is in");
        assertFalse(ProjectSync.goOn(21, 21, 5000, 2000), "none is in yet");
        assertFalse(ProjectSync.goOn(8, 21, 5000, 2000), "more than a third still sync");
        assertFalse(ProjectSync.goOn(2, 21, 1500, 2000), "a build finished a moment ago");
        assertTrue(ProjectSync.goOn(2, 21, 2000, 2000));
        assertFalse(ProjectSync.goOn(1, 2, 9000, 2000), "of two builds, the start waits for both");
        assertTrue(ProjectSync.goOn(1, 3, 2000, 2000));
    }

    @Test
    void aBuildStillSyncingIsAPlaceholderThenItsLastGoodModel(@TempDir Path dir) throws Exception {
        ProjectSync.Plan plan = new ProjectSync.Plan("slow", new File("/w/slow"), "8.14.3", null, new Sync.Options(), null);
        JsonObject placeholder = ProjectSync.placeholder(plan);
        assertEquals("syncing", placeholder.get("status").getAsString());
        assertTrue(placeholder.get("late").getAsBoolean());
        JsonObject model = new JsonObject();
        model.addProperty("rootDir", "/w");
        model.addProperty("syncMillis", 1);
        JsonArray builds = new JsonArray();
        builds.add(placeholder);
        model.add("gradleBuilds", builds);
        ProjectModel.assemble(model);
        assertEquals(0, model.getAsJsonArray("failedBuilds").size(), "still syncing is not failed");
        assertTrue(SnoutEngine.summary(model).contains("still syncing: slow"), SnoutEngine.summary(model));
        // With a model from an earlier sync, the build keeps it until its sync comes in.
        JsonObject last = new JsonObject();
        JsonArray lastBuilds = new JsonArray();
        JsonObject good = new JsonObject();
        good.addProperty("id", "slow");
        good.addProperty("rootDir", "/w/slow");
        good.addProperty("status", "synced");
        good.add("builds", new JsonArray());
        lastBuilds.add(good);
        last.add("gradleBuilds", lastBuilds);
        Path file = dir.resolve("model.json");
        Files.writeString(file, last.toString());
        assertEquals(1, Lsp.keepLastGood(model, file));
        JsonObject kept = model.getAsJsonArray("gradleBuilds").get(0).getAsJsonObject();
        assertEquals("stale", kept.get("status").getAsString());
        assertTrue(kept.get("late").getAsBoolean(), "and syncs again on the next start should it not come in");
    }

    @Test
    void namesBuildsByTheirPathInTheFolder() {
        File folder = new File("/w/scala");
        assertEquals("api", ProjectSync.buildId(folder, new File("/w/scala/api")));
        assertEquals("group/c", ProjectSync.buildId(folder, new File("/w/scala/group/c")));
        assertEquals("scala", ProjectSync.buildId(folder, folder));
        assertEquals("w", ProjectSync.buildId(folder, new File("/w")));
    }

    // ---- the project view

    private static String esc(Path p) {
        return p.toString().replace("\\", "\\\\");
    }

    private JsonObject twoServicesAndAFailedBuild() {
        String json = """
            { "modelVersion": 2, "rootDir": "%1$s", "gradleBuilds": [
              { "id": "one", "status": "synced", "rootDir": "%1$s/one", "gradleJavaHome": "/jdk17", "builds": [
                { "name": "one", "kind": "root", "rootDir": "%1$s/one", "errors": [], "projects": [
                  { "path": ":", "name": "one", "projectDir": "%1$s/one", "buildFile": "%1$s/one/build.gradle",
                    "buildDir": "%1$s/one/build",
                    "appliedScripts": { "scripts": ["%1$s/deps.gradle"] },
                    "testTasks": [ { "sourceSets": ["test"] } ],
                    "sourceSets": [
                      { "name": "main", "roots": [
                          { "dir": "%1$s/one/src/main/java", "kind": "source", "languages": ["scala", "java"] },
                          { "dir": "%1$s/one/../core/src/main/java", "kind": "source", "languages": ["scala", "java"] },
                          { "dir": "%1$s/one/src/main/resources", "kind": "resource", "languages": [] },
                          { "dir": "%1$s/one/src/main/java", "kind": "resource", "languages": [] } ],
                        "output": { "classesDirs": ["%1$s/one/build/classes/java/main"] },
                        "compileClasspath": [
                          { "kind": "module", "group": "g", "name": "lib", "version": "1.0", "file": "/repo/lib-1.0.jar" },
                          { "kind": "module", "group": "g", "name": "api", "version": "2.0", "file": "/repo/api-2.0.jar" } ],
                        "runtimeClasspath": [
                          { "kind": "sourceSetOutput", "sourceSet": "main", "files": [] },
                          { "kind": "module", "group": "g", "name": "lib", "version": "1.0", "file": "/repo/lib-1.0.jar" },
                          { "kind": "module", "group": "g", "name": "driver", "version": "3", "file": "/repo/driver-3.jar" } ] },
                      { "name": "test", "roots": [ { "dir": "%1$s/one/src/test/java", "kind": "source", "languages": ["java"] } ],
                        "compileClasspath": [
                          { "kind": "module", "group": "g", "name": "lib", "version": "1.0", "file": "/repo/lib-1.0.jar" },
                          { "kind": "module", "group": "t", "name": "junit", "version": "4", "file": "/repo/junit-4.jar" } ] } ] } ] } ] },
              { "id": "two", "status": "synced", "rootDir": "%1$s/two", "gradleJavaHome": "/jdk17", "builds": [
                { "name": "two", "kind": "root", "rootDir": "%1$s/two", "errors": [], "projects": [
                  { "path": ":", "name": "two", "projectDir": "%1$s/two",
                    "sourceSets": [ { "name": "main", "roots": [
                        { "dir": "%1$s/core/src/main/java", "kind": "source", "languages": ["java"] } ],
                      "compileClasspath": [
                        { "kind": "module", "group": "g", "name": "lib", "version": "1.0", "file": "/repo/lib-1.0.jar" } ] } ] } ] } ] },
              { "id": "old", "status": "failed", "rootDir": "%1$s/old", "gradleVersion": "6.0.1", "builds": [],
                "error": "long", "reason": "short" }
            ] }""".formatted(esc(dir));
        return JsonParser.parseString(json).getAsJsonObject();
    }

    private static JsonObject module(JsonObject model, String id) {
        for (JsonElement m : model.getAsJsonArray("modules")) {
            if (id.equals(m.getAsJsonObject().get("id").getAsString())) {
                return m.getAsJsonObject();
            }
        }
        throw new AssertionError("no module " + id);
    }

    private static List<String> ids(JsonArray refs) {
        List<String> out = new ArrayList<>();
        refs.forEach(r -> out.add(r.getAsJsonObject().get("id").getAsString()));
        return out;
    }

    @Test
    void typesSourceRootsAndSplitsDependenciesByScope() throws IOException {
        Files.createDirectories(dir.resolve("one"));
        Files.writeString(dir.resolve("one/build.gradle"), "");
        Files.writeString(dir.resolve("deps.gradle"), "");
        JsonObject model = twoServicesAndAFailedBuild();
        ProjectModel.assemble(model);
        JsonObject one = module(model, "one");
        List<String> roots = new ArrayList<>();
        for (JsonElement r : one.getAsJsonArray("sourceRoots")) {
            JsonObject root = r.getAsJsonObject();
            roots.add(dir.relativize(Path.of(root.get("dir").getAsString())) + " " + root.get("type").getAsString()
                + (root.get("shared").getAsBoolean() ? " shared" : ""));
        }
        assertEquals(List.of("one/src/main/java main", "core/src/main/java main shared", "one/src/test/java test",
            "one/src/main/resources resources"), roots, "a folder that is both source and resources is a source root");

        JsonObject deps = one.getAsJsonObject("dependencies");
        assertEquals(List.of("g:lib:1.0"), ids(deps.getAsJsonArray("compile")));
        assertEquals(List.of("g:api:2.0"), ids(deps.getAsJsonArray("provided")));
        assertEquals(List.of("g:driver:3"), ids(deps.getAsJsonArray("runtime")));
        assertEquals(List.of("t:junit:4"), ids(deps.getAsJsonArray("test")));
        assertTrue(one.getAsJsonArray("excludedDirs").toString().contains("one/build"));
        assertEquals("/jdk17", one.getAsJsonObject("jdk").get("javaHome").getAsString());

        assertEquals(4, model.getAsJsonArray("libraries").size(), "lib-1.0.jar once");
        JsonArray scripts = model.getAsJsonArray("gradleScripts").get(0).getAsJsonObject().getAsJsonArray("files");
        assertTrue(scripts.toString().contains("deps.gradle") && scripts.toString().contains("\"applied\""), scripts.toString());
    }

    @Test
    void listsSharedRootsWithTheirModulesAndTheFailedBuilds() {
        JsonObject model = twoServicesAndAFailedBuild();
        ProjectModel.assemble(model);
        JsonArray shared = model.getAsJsonArray("sharedSourceRoots");
        assertEquals(1, shared.size(), shared.toString());
        JsonObject core = shared.get(0).getAsJsonObject();
        assertEquals("core", core.get("name").getAsString());
        assertEquals(dir.resolve("core").toString(), core.get("contentRoot").getAsString());
        assertEquals(List.of("one", "two"), List.of(core.getAsJsonArray("modules").get(0).getAsString(),
            core.getAsJsonArray("modules").get(1).getAsString()));

        JsonArray failed = model.getAsJsonArray("failedBuilds");
        assertEquals(1, failed.size());
        assertEquals("old", failed.get(0).getAsJsonObject().get("build").getAsString());
        assertEquals("short", failed.get(0).getAsJsonObject().get("reason").getAsString());
        assertEquals("6.0.1", failed.get(0).getAsJsonObject().get("gradleVersion").getAsString());
    }

    @Test
    void aFolderInAModuleThatCompilesItIsOwnedNotShared() {
        JsonObject model = JsonParser.parseString("""
            { "rootDir": "/w", "gradleBuilds": [ { "id": "w", "status": "synced", "rootDir": "/w", "builds": [
              { "name": "w", "kind": "root", "rootDir": "/w", "errors": [], "projects": [
                { "path": ":lib", "name": "lib", "projectDir": "/w/lib", "sourceSets": [ { "name": "main",
                  "roots": [ { "dir": "/w/lib/src/main/java", "kind": "source", "languages": ["java"] } ] } ] },
                { "path": ":app", "name": "app", "projectDir": "/w/app", "sourceSets": [ { "name": "main",
                  "roots": [ { "dir": "/w/lib/src/main/java", "kind": "source", "languages": ["java"] } ] } ] } ] } ] } ] }
            """).getAsJsonObject();
        ProjectModel.assemble(model);
        assertEquals(0, model.getAsJsonArray("sharedSourceRoots").size());
        JsonObject root = module(model, "w:app").getAsJsonArray("sourceRoots").get(0).getAsJsonObject();
        assertEquals("w:lib", root.get("owner").getAsString());
    }
}
