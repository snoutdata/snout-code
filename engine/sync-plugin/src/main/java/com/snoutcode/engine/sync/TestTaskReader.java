// Copyright 2000-2024 JetBrains s.r.o. and contributors. Use of this source code is governed by the Apache 2.0 license.
// Adapted for SnoutCode from IntelliJ IDEA Community Edition,
// plugins/gradle/tooling-extension-impl/src/org/jetbrains/plugins/gradle/tooling/builder/ExternalTestsModelBuilderImpl.kt.
// Changes: ported from Kotlin to Java; also reports the test framework and JUnit Platform filters,
// and maps each task to source set names rather than source folders.
package com.snoutcode.engine.sync;

import org.gradle.api.Project;
import org.gradle.api.file.FileCollection;
import org.gradle.api.tasks.SourceSet;
import org.gradle.api.tasks.SourceSetContainer;
import org.gradle.api.tasks.testing.Test;

import java.io.File;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/** Reads the project's {@code Test} tasks: framework, filters and the source sets they run. */
final class TestTaskReader {
    private TestTaskReader() {
    }

    static List<Object> read(Project project, List<String> errors) {
        List<Object> out = new ArrayList<>();
        Map<String, String> classesDirToSourceSet = new LinkedHashMap<>();
        SourceSetContainer sourceSets = SourceSetReader.sourceSets(project);
        if (sourceSets != null) {
            for (SourceSet sourceSet : sourceSets) {
                for (File dir : sourceSet.getOutput().getClassesDirs().getFiles()) {
                    classesDirToSourceSet.put(dir.getAbsolutePath(), sourceSet.getName());
                }
            }
        }
        for (Test test : new ArrayList<>(project.getTasks().withType(Test.class))) {
            try {
                Map<String, Object> t = new LinkedHashMap<>();
                t.put("name", test.getName());
                t.put("path", test.getPath());
                t.put("framework", framework(test));
                List<String> classesDirs = new ArrayList<>();
                List<String> sourceSetNames = new ArrayList<>();
                Object dirs = Reflect.call(test, "getTestClassesDirs");
                if (dirs instanceof FileCollection) {
                    for (File dir : ((FileCollection) dirs).getFiles()) {
                        classesDirs.add(dir.getAbsolutePath());
                        String name = classesDirToSourceSet.get(dir.getAbsolutePath());
                        if (name != null && !sourceSetNames.contains(name)) {
                            sourceSetNames.add(name);
                        }
                    }
                }
                t.put("sourceSets", sourceSetNames);
                t.put("testClassesDirs", classesDirs);
                Object options = Reflect.call(test, "getOptions");
                if (Reflect.isInstance(options, "org.gradle.api.tasks.testing.junitplatform.JUnitPlatformOptions")) {
                    Map<String, Object> filters = new LinkedHashMap<>();
                    filters.put("includeTags", Reflect.strings(Reflect.call(options, "getIncludeTags")));
                    filters.put("excludeTags", Reflect.strings(Reflect.call(options, "getExcludeTags")));
                    filters.put("includeEngines", Reflect.strings(Reflect.call(options, "getIncludeEngines")));
                    filters.put("excludeEngines", Reflect.strings(Reflect.call(options, "getExcludeEngines")));
                    t.put("junitPlatform", filters);
                }
                out.add(t);
            } catch (Throwable e) {
                errors.add("test task " + test.getPath() + ": " + e);
            }
        }
        return out;
    }

    /** junit-platform, junit4, testng, or the framework class name when it is something else. */
    static String framework(Test test) {
        Object framework = Reflect.unwrap(Reflect.call(test, "getTestFramework"));
        if (framework == null) {
            return null;
        }
        String name = framework.getClass().getName();
        if (name.contains("JUnitPlatformTestFramework")) {
            return "junit-platform";
        }
        if (name.contains("JUnitTestFramework")) {
            return "junit4";
        }
        if (name.contains("TestNGTestFramework")) {
            return "testng";
        }
        return name;
    }
}
