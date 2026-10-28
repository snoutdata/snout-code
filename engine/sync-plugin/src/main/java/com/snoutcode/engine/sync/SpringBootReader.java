package com.snoutcode.engine.sync;

import org.gradle.api.Plugin;
import org.gradle.api.Project;
import org.gradle.api.Task;
import org.gradle.api.file.SourceDirectorySet;
import org.gradle.api.tasks.SourceSet;
import org.gradle.api.tasks.SourceSetContainer;

import java.io.File;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.FileVisitResult;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.SimpleFileVisitor;
import java.nio.file.attribute.BasicFileAttributes;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * Spring Boot facts: the plugin version, the main class wherever the build states it, and the
 * {@code bootRun} task. Spring Boot resolves an unset main class by scanning compiled classes
 * (the resolveMainClassName task), which sync must not run, so the reader also scans the main
 * source set for {@code @SpringBootApplication} classes and reports them as candidates.
 */
final class SpringBootReader {
    static final String PLUGIN_ID = "org.springframework.boot";

    private static final Pattern PACKAGE = Pattern.compile("^\\s*package\\s+([\\w.]+)", Pattern.MULTILINE);
    private static final Pattern BOOT_APP = Pattern.compile(
        "@SpringBootApplication\\b[^{]*?\\b(?:class|object)\\s+(\\w+)", Pattern.DOTALL);
    private static final int MAX_FILES = 20000;

    private SpringBootReader() {
    }

    static Map<String, Object> read(Project project, List<String> errors) {
        if (!project.getPluginManager().hasPlugin(PLUGIN_ID)) {
            return null;
        }
        Map<String, Object> out = new LinkedHashMap<>();
        out.put("pluginVersion", pluginVersion(project));
        Object extension = project.getExtensions().findByName("springBoot");
        Object mainClass = Reflect.unwrap(Reflect.property(extension, "mainClass"));
        if (mainClass == null) {
            mainClass = Reflect.unwrap(Reflect.property(extension, "mainClassName"));
        }
        out.put("mainClass", mainClass == null ? null : mainClass.toString());
        out.put("bootJarMainClass", taskMainClass(project, "bootJar"));
        out.put("jarManifestMainClass", manifestMainClass(project));

        Task bootRun = project.getTasks().findByName("bootRun");
        if (bootRun != null) {
            Map<String, Object> run = new LinkedHashMap<>();
            run.put("task", bootRun.getPath());
            run.put("mainClass", taskMainClass(project, "bootRun"));
            run.put("args", Reflect.strings(Reflect.call(bootRun, "getArgs")));
            run.put("jvmArgs", Reflect.strings(Reflect.call(bootRun, "getJvmArgs")));
            File workingDir = Reflect.file(Reflect.call(bootRun, "getWorkingDir"));
            run.put("workingDir", workingDir == null ? null : workingDir.getAbsolutePath());
            Object systemProperties = Reflect.call(bootRun, "getSystemProperties");
            Map<String, Object> props = new LinkedHashMap<>();
            if (systemProperties instanceof Map) {
                for (Map.Entry<?, ?> e : ((Map<?, ?>) systemProperties).entrySet()) {
                    props.put(String.valueOf(e.getKey()), String.valueOf(e.getValue()));
                }
            }
            run.put("systemProperties", props);
            out.put("bootRun", run);
        } else {
            out.put("bootRun", null);
        }
        try {
            out.put("mainClassCandidates", scanForApplications(project));
        } catch (Throwable e) {
            errors.add("Spring Boot main class scan: " + e);
        }
        return out;
    }

    private static String pluginVersion(Project project) {
        for (Plugin<?> plugin : project.getPlugins()) {
            Class<?> c = plugin.getClass();
            if (c.getName().startsWith("org.springframework.boot.gradle.plugin.SpringBootPlugin")) {
                Package p = c.getPackage();
                return p == null ? null : p.getImplementationVersion();
            }
        }
        return null;
    }

    /** A task's mainClass when it is set or can be computed without running a task. */
    private static String taskMainClass(Project project, String taskName) {
        Task task = project.getTasks().findByName(taskName);
        if (task == null) {
            return null;
        }
        Object value = Reflect.call(task, "getMainClass");
        if (value == null) {
            value = Reflect.call(task, "getMainClassName");
        }
        if (value == null) {
            value = Reflect.call(task, "getMain"); // JavaExec before Gradle 6.4 (Spring Boot 1.x bootRun)
        }
        try {
            Object v = Reflect.unwrap(value);
            return v == null ? null : v.toString();
        } catch (Throwable e) {
            return null;
        }
    }

    private static String manifestMainClass(Project project) {
        Task jar = project.getTasks().findByName("jar");
        Object attributes = Reflect.call(Reflect.call(jar, "getManifest"), "getAttributes");
        if (attributes instanceof Map) {
            Object v = ((Map<?, ?>) attributes).get("Main-Class");
            return v == null ? null : v.toString();
        }
        return null;
    }

    static List<String> scanForApplications(Project project) throws IOException {
        Set<File> dirs = new LinkedHashSet<>();
        SourceSetContainer sourceSets = SourceSetReader.sourceSets(project);
        SourceSet main = sourceSets == null ? null : sourceSets.findByName(SourceSet.MAIN_SOURCE_SET_NAME);
        if (main == null) {
            return new ArrayList<>();
        }
        for (String language : SourceSetReader.LANGUAGES) {
            SourceDirectorySet set = SourceSetReader.languageDirs(main, language);
            if (set != null) {
                dirs.addAll(set.getSrcDirs());
            }
        }
        final List<String> found = new ArrayList<>();
        final int[] visited = {0};
        for (File dir : dirs) {
            if (!dir.isDirectory()) {
                continue;
            }
            Files.walkFileTree(dir.toPath(), new SimpleFileVisitor<Path>() {
                @Override
                public FileVisitResult visitFile(Path file, BasicFileAttributes attrs) throws IOException {
                    String name = file.getFileName().toString();
                    if (!(name.endsWith(".java") || name.endsWith(".kt") || name.endsWith(".scala")
                        || name.endsWith(".groovy"))) {
                        return FileVisitResult.CONTINUE;
                    }
                    if (++visited[0] > MAX_FILES) {
                        return FileVisitResult.TERMINATE;
                    }
                    String text = new String(Files.readAllBytes(file), StandardCharsets.UTF_8);
                    if (!text.contains("@SpringBootApplication")) {
                        return FileVisitResult.CONTINUE;
                    }
                    Matcher app = BOOT_APP.matcher(text);
                    if (app.find()) {
                        Matcher pkg = PACKAGE.matcher(text);
                        String fqcn = pkg.find() ? pkg.group(1) + "." + app.group(1) : app.group(1);
                        if (!found.contains(fqcn)) {
                            found.add(fqcn);
                        }
                    }
                    return FileVisitResult.CONTINUE;
                }
            });
        }
        return found;
    }
}
