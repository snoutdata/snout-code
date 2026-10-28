package com.snoutcode.engine.lsp;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.TestInstance;

import javax.tools.JavaCompiler;
import javax.tools.ToolProvider;
import java.io.IOException;
import java.io.OutputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.jar.JarEntry;
import java.util.jar.JarOutputStream;
import java.util.stream.Stream;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/**
 * Runs {@code snout-engine lsp} on a folder of three independent Gradle builds: {@code alpha} and
 * {@code beta} both compile the folder {@code shared} (not a Gradle project) into themselves, one
 * through its Scala source set, and get its library from an applied script; {@code broken} fails.
 * Checks that the two good builds import with no errors, that the shared code exists once in
 * jdtls (references list each use once, navigation from both builds lands on the one copy) with
 * the libraries its users give it, and that the model reports the failed build.
 */
@TestInstance(TestInstance.Lifecycle.PER_CLASS)
class LspMultiBuildTest {
    private Path folder;
    private List<String> before;
    private JsonObject report;

    @BeforeAll
    void run() throws Exception {
        Path scratch = Path.of(System.getProperty("snout.test.scratch")).resolve("multi-" + System.nanoTime());
        folder = scratch.resolve("multi-build");
        LspEndToEndTest.copy(Path.of(System.getProperty("snout.test.fixtures"), "multi-build"), folder);
        helperJar(folder.resolve("libs/helper.jar"), scratch.resolve("helper"));
        before = LspEndToEndTest.listing(folder);

        String alpha = "alpha/src/main/java/demo/alpha/Alpha.java";
        String beta = "beta/src/main/java/demo/beta/Beta.java";
        String text = "shared/src/main/java/demo/shared/Text.java";
        String java = System.getProperty("java.home");
        JsonObject script = JsonParser.parseString("""
            {
              "engineArgs": ["--cache-dir", "%1$s", "--gradle-version", "%2$s", "--java-home", "%3$s", "--no-sources"],
              "env": { "JAVA_HOME": "%3$s" },
              "open": ["%4$s", "%5$s", "%6$s"],
              "checks": [
                { "name": "definition from alpha into the shared folder", "kind": "definition", "file": "%4$s",
                  "find": "Text.shout", "offset": 5, "expect": "shared/src/main/java/demo/shared/Text.java", "single": true },
                { "name": "definition from beta into the shared folder", "kind": "definition", "file": "%5$s",
                  "find": "Text.shout", "offset": 5, "expect": "shared/src/main/java/demo/shared/Text.java", "single": true },
                { "name": "references list each use once", "kind": "references", "file": "%6$s",
                  "find": "shout(", "min": 3, "max": 3, "distinct": true },
                { "name": "the shared code sees its users' library", "kind": "definition", "file": "%6$s",
                  "find": "Helper.upper", "offset": 7, "expect": "Helper", "single": true }
              ]
            }""".formatted(
            LspEndToEndTest.json(scratch.resolve("cache").toString()), System.getProperty("snout.test.gradleVersion"),
            LspEndToEndTest.json(java), alpha, beta, text)).getAsJsonObject();
        report = Driver.run(Path.of(System.getProperty("snout.test.engine")), folder, script, scratch.resolve("stderr.log"));
        Files.writeString(scratch.resolve("report.json"), report.toString());
    }

    @Test
    void importsTheGoodBuildsWithoutErrors() {
        JsonObject diagnostics = report.getAsJsonObject("diagnostics");
        assertEquals(0, diagnostics.get("errors").getAsInt(), report.toString());
        assertEquals(0, diagnostics.getAsJsonArray("nonJavaErrors").size(), report.toString());
        List<String> statuses = new ArrayList<>();
        report.getAsJsonArray("statuses").forEach(e -> statuses.add(e.getAsString()));
        assertTrue(statuses.contains("ProjectStatus: OK"), statuses.toString());
    }

    @Test
    void sharedCodeExistsOnce() {
        JsonArray checks = report.getAsJsonArray("checks");
        assertEquals(4, checks.size());
        for (JsonElement c : checks) {
            assertTrue(c.getAsJsonObject().get("pass").getAsBoolean(), c.toString());
        }
    }

    @Test
    void modelHasModulesSharedRootsLibrariesScriptsAndTheFailedBuild() throws IOException {
        JsonObject model = model();
        assertEquals(2, model.get("modelVersion").getAsInt());
        List<String> builds = new ArrayList<>();
        model.getAsJsonArray("gradleBuilds").forEach(b -> builds.add(b.getAsJsonObject().get("id").getAsString()
            + " " + b.getAsJsonObject().get("status").getAsString()));
        assertEquals(List.of("alpha synced", "beta synced", "broken failed"), builds);

        List<String> modules = new ArrayList<>();
        model.getAsJsonArray("modules").forEach(m -> modules.add(m.getAsJsonObject().get("id").getAsString()));
        assertEquals(List.of("alpha", "beta"), modules);

        JsonArray shared = model.getAsJsonArray("sharedSourceRoots");
        assertEquals(1, shared.size(), shared.toString());
        JsonObject root = shared.get(0).getAsJsonObject();
        assertTrue(root.get("dir").getAsString().endsWith("shared/src/main/java"), root.toString());
        assertEquals("shared", root.get("name").getAsString());
        assertEquals("main", root.get("type").getAsString());
        assertEquals("[\"alpha\",\"beta\"]", root.getAsJsonArray("modules").toString());

        long helpers = 0;
        for (JsonElement l : model.getAsJsonArray("libraries")) {
            if (l.getAsJsonObject().get("file").getAsString().endsWith("helper.jar")) {
                helpers++;
            }
        }
        assertEquals(1, helpers, "each jar is listed once");

        for (JsonElement s : model.getAsJsonArray("gradleScripts")) {
            String files = s.getAsJsonObject().getAsJsonArray("files").toString();
            assertTrue(files.contains("settings.gradle") && files.contains("build.gradle"), files);
            if (!s.getAsJsonObject().get("build").getAsString().equals("broken")) {
                assertTrue(files.contains("shared-deps.gradle"), files);
            }
        }

        JsonArray failed = model.getAsJsonArray("failedBuilds");
        assertEquals(1, failed.size());
        JsonObject broken = failed.get(0).getAsJsonObject();
        assertEquals("broken", broken.get("build").getAsString());
        assertEquals(System.getProperty("snout.test.gradleVersion"), broken.get("gradleVersion").getAsString());
        assertTrue(broken.get("reason").getAsString().contains("This build is broken on purpose."), broken.toString());
    }

    @Test
    void leavesTheFolderUntouched() throws IOException {
        assertEquals(before, LspEndToEndTest.listing(folder));
    }

    private JsonObject model() throws IOException {
        try (Stream<Path> files = Files.walk(folder.getParent().resolve("cache/jdtls"))) {
            Path model = files.filter(p -> p.getFileName().toString().equals("model.json")).findFirst().orElseThrow();
            return JsonParser.parseString(Files.readString(model)).getAsJsonObject();
        }
    }

    /** A jar with one class, {@code demo.helper.Helper}, that only the applied script puts on the classpath. */
    private static void helperJar(Path jar, Path work) throws IOException {
        Path src = work.resolve("src/demo/helper/Helper.java");
        Path classes = work.resolve("classes");
        Files.createDirectories(src.getParent());
        Files.createDirectories(classes);
        Files.writeString(src, """
            package demo.helper;

            public final class Helper {
                private Helper() {
                }

                public static String upper(String s) {
                    return s.toUpperCase();
                }
            }
            """);
        JavaCompiler javac = ToolProvider.getSystemJavaCompiler();
        int exit = javac.run(null, null, null, "--release", "17", "-d", classes.toString(), src.toString());
        assertEquals(0, exit, "javac failed");
        Files.createDirectories(jar.getParent());
        try (OutputStream out = Files.newOutputStream(jar); JarOutputStream zip = new JarOutputStream(out)) {
            zip.putNextEntry(new JarEntry("demo/helper/Helper.class"));
            zip.write(Files.readAllBytes(classes.resolve("demo/helper/Helper.class")));
            zip.closeEntry();
        }
    }
}
