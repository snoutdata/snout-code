package com.snoutcode.engine.jdtls;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.io.File;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.stream.Collectors;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

class ImportPlanTest {
    @TempDir
    Path dir;

    private Path root;
    private Path tools;

    @BeforeEach
    void layout() throws IOException {
        root = dir.resolve("demo");
        tools = dir.resolve("tools");
        for (String d : List.of("demo/app/src/main/java", "demo/app/src/main/java/gen", "demo/app/src/test/java",
            "demo/lib/src/main/java", "shared/src/main/java", "tools/src/main/java", "out/lib/classes/java/main",
            "out/app/generated", "scala/0")) {
            Files.createDirectories(dir.resolve(d));
        }
        for (String f : List.of("repo/guava.jar", "repo/guava-sources.jar", "repo/junit.jar", "repo/local.jar")) {
            Files.createDirectories(dir.resolve(f).getParent());
            Files.writeString(dir.resolve(f), "");
        }
    }

    private String p(String relative) {
        return dir.resolve(relative).toString().replace("\\", "\\\\");
    }

    private JsonObject model() {
        String json = """
            {
              "rootDir": "%1$s", "gradleJavaHome": "/jdk/gradle",
              "builds": [
                { "name": "demo", "kind": "root", "rootDir": "%1$s", "projects": [
                  { "path": ":", "name": "demo", "projectDir": "%1$s", "sourceSets": [] },
                  { "path": ":app", "name": "app", "projectDir": "%2$s",
                    "java": { "sourceCompatibility": "11", "targetCompatibility": "11" },
                    "testTasks": [ { "sourceSets": ["test"] } ],
                    "sourceSets": [
                      { "name": "test",
                        "javaSourceDirs": ["%3$s"], "roots": [],
                        "output": { "classesDirs": ["%13$s/app-test"] },
                        "compileClasspath": [
                          { "kind": "sourceSetOutput", "sourceSet": "main", "files": [] },
                          { "kind": "module", "file": "%7$s", "sources": "%8$s" },
                          { "kind": "module", "file": "%9$s" }
                        ] },
                      { "name": "main",
                        "javaSourceDirs": ["%4$s", "%5$s", "%6$s", "%14$s"],
                        "roots": [ { "dir": "%15$s", "kind": "generated", "languages": ["java"] } ],
                        "output": { "classesDirs": ["%13$s/app-main"] },
                        "compile": { "java": { "sourceCompatibility": "17", "targetCompatibility": "17",
                                     "release": null, "encoding": "UTF-8", "compilerArgs": ["-parameters"],
                                     "toolchain": { "javaHome": "/jdk/17" } } },
                        "scalaBridge": { "folders": [ { "classesDir": "%16$s", "sourceDir": "%5$s" } ] },
                        "compileClasspath": [
                          { "kind": "project", "build": ":", "path": ":lib", "files": [] },
                          { "kind": "project", "build": ":tools", "path": ":", "files": [] },
                          { "kind": "module", "file": "%7$s", "sources": "/missing/sources.jar" },
                          { "kind": "file", "file": "%10$s" },
                          { "kind": "file", "file": "%11$s" },
                          { "kind": "file", "file": "/missing/gone.jar" }
                        ] } ] },
                  { "path": ":lib", "name": "lib", "projectDir": "%12$s",
                    "sourceSets": [ { "name": "main", "javaSourceDirs": ["%17$s"], "roots": [],
                                      "output": { "classesDirs": ["%11$s"] }, "compileClasspath": [] } ] }
                ], "errors": [] },
                { "name": "tools", "kind": "included", "rootDir": "%18$s", "projects": [
                  { "path": ":", "name": "app", "projectDir": "%18$s",
                    "sourceSets": [ { "name": "main", "javaSourceDirs": ["%19$s"], "roots": [],
                                      "output": { "classesDirs": [] }, "compileClasspath": [] } ] }
                ], "errors": [] }
              ]
            }""".formatted(
            p("demo"), p("demo/app"), p("demo/app/src/test/java"),
            p("demo/app/src/main/java"), p("shared/src/main/java"), p("demo/app/src/main/java/gen"),
            p("repo/guava.jar"), p("repo/guava-sources.jar"), p("repo/junit.jar"),
            p("repo/local.jar"), p("out/lib/classes/java/main"), p("demo/lib"), p("out"),
            p("demo/app/src/main/missing"), p("out/app/generated"), p("scala/0"),
            p("demo/lib/src/main/java"), p("tools"), p("tools/src/main/java"));
        return JsonParser.parseString(json).getAsJsonObject();
    }

    private static ImportPlan.Project project(ImportPlan plan, String name) {
        return plan.projects.stream().filter(p -> p.name.equals(name)).findFirst().orElseThrow();
    }

    @Test
    void makesOneProjectPerGradleProjectWithSources() {
        ImportPlan plan = ImportPlan.from(model());
        // The included build's root project is also called "app", so it gets the build's name.
        assertEquals(List.of("app", "lib", "tools"), plan.projects.stream().map(p -> p.name).toList());
        assertEquals(4, plan.projectDirs.size(), "the root project without sources counts as a directory too");
        assertEquals(root.toFile(), plan.rootDir);
    }

    @Test
    void linksSourceRootsMainFirstAndExcludesNestedOnes() {
        ImportPlan.Project app = project(ImportPlan.from(model()), "app");
        List<String> links = app.sources.stream().map(s -> s.link + (s.test ? " test" : "") + (s.generated ? " generated" : ""))
            .toList();
        assertEquals(List.of("src-main-java", "shared-src-main-java", "src-main-java-gen", "out-app-generated generated",
            "src-test-java test"), links, "missing folders are skipped");
        assertEquals(List.of("gen/"), app.sources.get(0).exclusions);
    }

    @Test
    void namesLinksOutsideTheProjectFromTheCommonAncestor() {
        File project = new File("/work/demo/app");
        assertEquals("src-main-java", ImportPlan.linkName(project, new File("/work/demo/app/src/main/java")));
        assertEquals("core-src-main-java", ImportPlan.linkName(project, new File("/work/demo/core/src/main/java")));
        assertEquals("src", ImportPlan.linkName(project, project));
    }

    @Test
    void mapsTheClasspath() {
        ImportPlan.Project app = project(ImportPlan.from(model()), "app");
        List<String> deps = app.dependencies.stream().map(d -> d.project != null ? "project " + d.project
            : new File(String.valueOf(d.file)).getName() + (d.sources != null ? " +sources" : "")
                + (d.exported ? " exported" : "") + (d.test ? " test" : "")).collect(Collectors.toList());
        assertEquals(List.of(
            "0 +sources exported",     // the Scala bridge
            "project lib",
            "project tools",           // ":tools" names the included build
            "guava.jar +sources",      // the test source set's lookup found the source jar
            "local.jar",
            "junit.jar test"           // only the test source set uses it
        ), deps, "lib's classes folder maps to the project, and missing files are dropped");
    }

    @Test
    void readsCompilerSettingsFromTheMainJavaCompile() {
        ImportPlan.JavaSettings java = project(ImportPlan.from(model()), "app").java;
        assertEquals("17", java.source);
        assertEquals("17", java.target);
        assertNull(java.release);
        assertEquals("UTF-8", java.encoding);
        assertTrue(java.parameters);
        assertFalse(java.preview);
        assertEquals("/jdk/17", java.javaHome);
        assertEquals("/jdk/gradle", project(ImportPlan.from(model()), "lib").java.javaHome);
    }

    @Test
    void keepsTheSyncErrorOfAnEmptyModel() {
        JsonObject model = new JsonObject();
        model.addProperty("rootDir", root.toString());
        model.add("builds", new JsonArray());
        model.addProperty("syncError", "boom");
        ImportPlan plan = ImportPlan.from(model);
        assertEquals("boom", plan.syncError);
        assertTrue(plan.projects.isEmpty());
    }

    @Test
    void relatesRootFoldersBothWays() {
        File a = root.toFile();
        assertTrue(SnoutProjectImporter.related(a, a));
        assertTrue(SnoutProjectImporter.related(dir.toFile(), a));
        assertTrue(SnoutProjectImporter.related(new File(a, "app"), a));
        assertFalse(SnoutProjectImporter.related(tools.toFile(), a));
    }
}
