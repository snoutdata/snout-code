package com.snoutcode.engine.sync;

import org.gradle.api.Project;
import org.gradle.api.file.SourceDirectorySet;
import org.gradle.api.tasks.SourceSet;
import org.gradle.api.tasks.SourceSetContainer;
import org.gradle.api.tasks.testing.Test;
import org.gradle.testfixtures.ProjectBuilder;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.io.TempDir;

import java.io.File;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.Map;
import java.util.stream.Collectors;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * Builds the model of an in-memory project shaped like the reference build: Java compiled
 * jointly by the Scala source set, a source folder outside the project, a project dependency
 * and a local jar.
 */
class ProjectModelBuilderTest {

    @TempDir
    File dir;

    private Project root;
    private Map<String, Object> model;

    @BeforeEach
    void setUp() {
        root = ProjectBuilder.builder().withProjectDir(dir).withName("web").build();
        Project lib = ProjectBuilder.builder().withParent(root).withName("lib")
            .withProjectDir(new File(dir, "lib")).build();
        lib.getPluginManager().apply("java-library");

        root.getPluginManager().apply("java");
        root.getPluginManager().apply("scala");
        SourceSet main = root.getExtensions().getByType(SourceSetContainer.class).getByName("main");
        main.getJava().setSrcDirs(Collections.emptyList());
        SourceDirectorySet scala = (SourceDirectorySet) main.getExtensions().getByName("scala");
        scala.setSrcDirs(Arrays.asList("src/main/java", "src/main/scala", "../shared/src/main/java"));
        root.getDependencies().add("implementation", root.project(":lib"));
        root.getDependencies().add("implementation", root.files("lib/local.jar"));
        ((Test) root.getTasks().getByName("test")).useJUnitPlatform();

        model = ProjectModelBuilder.build(root);
    }

    @org.junit.jupiter.api.Test
    void reportsScalaSourceSetCompilingJavaJointly() {
        Map<String, Object> main = sourceSet("main");
        Map<String, Object> sources = map(main.get("sources"));
        assertEquals(Collections.emptyList(), map(sources.get("java")).get("srcDirs"));
        assertEquals(Arrays.asList(path("src/main/java"), path("src/main/scala"), path("../shared/src/main/java")),
            map(sources.get("scala")).get("srcDirs"));
        assertEquals(map(sources.get("scala")).get("srcDirs"), main.get("javaSourceDirs"));

        Map<String, Object> shared = list(main.get("roots")).stream().map(ProjectModelBuilderTest::map)
            .filter(r -> r.get("dir").equals(path("../shared/src/main/java")))
            .findFirst().orElseThrow(AssertionError::new);
        assertEquals("source", shared.get("kind"));
        assertEquals(Arrays.asList("scala", "java"), shared.get("languages"));
        assertEquals(Collections.singletonList("compileScala"), shared.get("compiledBy"));
    }

    @org.junit.jupiter.api.Test
    void keepsProjectDependenciesAsReferences() {
        List<Map<String, Object>> classpath = list(sourceSet("main").get("compileClasspath")).stream()
            .map(ProjectModelBuilderTest::map).collect(Collectors.toList());
        Map<String, Object> project = classpath.stream().filter(e -> "project".equals(e.get("kind")))
            .findFirst().orElseThrow(AssertionError::new);
        assertEquals(":lib", project.get("path"));
        assertTrue(classpath.stream().anyMatch(e -> "file".equals(e.get("kind"))
            && e.get("file").equals(path("lib/local.jar"))), classpath.toString());
    }

    @org.junit.jupiter.api.Test
    void testClasspathSeesMainOutput() {
        List<Map<String, Object>> classpath = list(sourceSet("test").get("compileClasspath")).stream()
            .map(ProjectModelBuilderTest::map).collect(Collectors.toList());
        Map<String, Object> output = classpath.get(0);
        assertEquals("sourceSetOutput", output.get("kind"));
        assertEquals("main", output.get("sourceSet"));
    }

    @org.junit.jupiter.api.Test
    void reportsTestFrameworkAndPlugins() {
        Map<String, Object> test = map(list(model.get("testTasks")).get(0));
        assertEquals(":test", test.get("path"));
        assertEquals("junit-platform", test.get("framework"));
        assertEquals(Collections.singletonList("test"), test.get("sourceSets"));

        List<Object> classes = list(model.get("plugins")).stream().map(p -> map(p).get("class")).collect(Collectors.toList());
        assertTrue(classes.contains("org.gradle.api.plugins.scala.ScalaPlugin"), classes.toString());
        assertTrue(list(model.get("errors")).isEmpty(), String.valueOf(model.get("errors")));
    }

    @org.junit.jupiter.api.Test
    void graphsSayWhereEachDependencyIsDeclared() {
        Map<String, Object> graphs = map(sourceSet("main").get("dependencyGraph"));
        Map<String, Object> compile = map(graphs.get("compileClasspath"));
        Map<String, Object> edge = list(compile.get("roots")).stream().map(ProjectModelBuilderTest::map)
            .filter(e -> e.containsKey("to")).findFirst().orElseThrow(AssertionError::new);
        Map<String, Object> node = map(list(compile.get("nodes")).get((Integer) edge.get("to")));
        assertEquals("project", node.get("kind"));
        assertEquals(":lib", node.get("path"));
        assertEquals("implementation", edge.get("declaredIn"));
    }

    /**
     * Two versions of one library asked for, the newer one wins: the node says it won a conflict
     * and the edge that asked for the older one keeps its version.
     */
    @org.junit.jupiter.api.Test
    void graphsMarkVersionConflicts(@TempDir File other) throws Exception {
        File repo = new File(other, "repo");
        pom(repo, "demo", "util", "1.0", null);
        pom(repo, "demo", "util", "2.0", null);
        pom(repo, "demo", "client", "1.0", "<dependency><groupId>demo</groupId><artifactId>util</artifactId><version>1.0</version></dependency>");
        Project p = ProjectBuilder.builder().withProjectDir(new File(other, "app")).withName("app").build();
        p.getPluginManager().apply("java");
        p.getRepositories().maven(r -> r.setUrl(repo.toURI()));
        p.getDependencies().add("implementation", "demo:client:1.0");
        p.getDependencies().add("runtimeOnly", "demo:util:2.0");
        Map<String, Object> built = ProjectModelBuilder.build(p);
        Map<String, Object> main = list(built.get("sourceSets")).stream().map(ProjectModelBuilderTest::map)
            .filter(s -> "main".equals(s.get("name"))).findFirst().orElseThrow(AssertionError::new);
        Map<String, Object> runtime = map(map(main.get("dependencyGraph")).get("runtimeClasspath"));
        List<Map<String, Object>> nodes = list(runtime.get("nodes")).stream().map(ProjectModelBuilderTest::map)
            .collect(Collectors.toList());
        int util = -1;
        for (int i = 0; i < nodes.size(); i++) {
            if ("util".equals(nodes.get(i).get("name"))) {
                util = i;
            }
        }
        assertEquals("2.0", nodes.get(util).get("version"));
        assertEquals("conflict", nodes.get(util).get("reason"));
        Map<String, Object> client = nodes.stream().filter(n -> "client".equals(n.get("name"))).findFirst()
            .orElseThrow(AssertionError::new);
        Map<String, Object> asked = map(list(client.get("deps")).get(0));
        assertEquals(util, asked.get("to"));
        assertEquals("1.0", asked.get("requested"));
        List<String> declared = list(runtime.get("roots")).stream().map(e -> String.valueOf(map(e).get("declaredIn")))
            .collect(Collectors.toList());
        assertEquals(Arrays.asList("implementation", "runtimeOnly"), declared);
    }

    private static void pom(File repo, String group, String name, String version, String deps) throws Exception {
        File dir = new File(repo, group + "/" + name + "/" + version);
        dir.mkdirs();
        String pom = "<project><modelVersion>4.0.0</modelVersion><groupId>" + group + "</groupId><artifactId>" + name
            + "</artifactId><version>" + version + "</version>"
            + (deps == null ? "" : "<dependencies>" + deps + "</dependencies>") + "</project>";
        java.nio.file.Files.write(new File(dir, name + "-" + version + ".pom").toPath(), pom.getBytes("UTF-8"));
        try (java.util.zip.ZipOutputStream z = new java.util.zip.ZipOutputStream(
            new java.io.FileOutputStream(new File(dir, name + "-" + version + ".jar")))) {
            z.putNextEntry(new java.util.zip.ZipEntry("META-INF/MANIFEST.MF"));
            z.closeEntry();
        }
    }

    @org.junit.jupiter.api.Test
    void rootProjectCarriesBuildInfo() {
        Map<String, Object> build = map(model.get("build"));
        assertNotNull(build.get("gradleVersion"));
        assertEquals(root.getRootDir().getAbsolutePath(), build.get("rootDir"));
    }

    private String path(String relative) {
        return root.file(relative).getAbsolutePath();
    }

    private Map<String, Object> sourceSet(String name) {
        return list(model.get("sourceSets")).stream().map(ProjectModelBuilderTest::map)
            .filter(s -> name.equals(s.get("name"))).findFirst().orElseThrow(AssertionError::new);
    }

    @SuppressWarnings("unchecked")
    private static Map<String, Object> map(Object o) {
        return (Map<String, Object>) o;
    }

    @SuppressWarnings("unchecked")
    private static List<Object> list(Object o) {
        return (List<Object>) o;
    }
}
