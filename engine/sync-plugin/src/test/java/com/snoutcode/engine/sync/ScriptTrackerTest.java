package com.snoutcode.engine.sync;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.io.File;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.Arrays;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;

class ScriptTrackerTest {

    @TempDir
    File dir;

    @Test
    void scansApplyFromInGroovyAndKotlinSyntax() throws IOException {
        File project = new File(dir, "web");
        project.mkdirs();
        File build = new File(project, "build.gradle");
        Files.write(build.toPath(), String.join("\n",
            "apply from: \"../deps.gradle\"",
            "apply(from: 'gradle/a.gradle')",
            "apply(from = \"gradle/b.gradle.kts\")",
            "apply from: file('gradle/c.gradle')",
            "apply from: \"$rootDir/dynamic.gradle\"",
            "apply from: 'https://example.com/remote.gradle'",
            "apply plugin: 'java'").getBytes(StandardCharsets.UTF_8));

        List<String> scripts = ScriptTracker.scanBuildFile(build);

        assertEquals(Arrays.asList(
            new File(dir, "deps.gradle").getCanonicalPath(),
            new File(project, "gradle/a.gradle").getCanonicalPath(),
            new File(project, "gradle/b.gradle.kts").getCanonicalPath(),
            new File(project, "gradle/c.gradle").getCanonicalPath(),
            "https://example.com/remote.gradle"), scripts);
    }

    @Test
    void missingBuildFileHasNoScripts() {
        assertEquals(0, ScriptTracker.scanBuildFile(new File(dir, "none.gradle")).size());
    }
}
