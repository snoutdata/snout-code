// Copyright 2000-2024 JetBrains s.r.o. and contributors. Use of this source code is governed by the Apache 2.0 license.
// Adapted for SnoutCode from IntelliJ IDEA Community Edition,
// plugins/gradle/tooling-extension-impl/src/com/intellij/gradle/toolingExtension/impl/model/sourceSetModel/GradleSourceSetModelBuilder.java,
// plugins/gradle/tooling-extension-impl/src/org/jetbrains/plugins/gradle/tooling/builder/AnnotationProcessingModelBuilder.java
// and plugins/gradle/tooling-extension-impl/src/com/intellij/gradle/toolingExtension/impl/util/javaPluginUtil/*.
// Changes: one reader for source directories per language, generated sources, outputs, compiler
// settings and classpaths, emitting plain maps; all version-specific Gradle API goes through Reflect.
package com.snoutcode.engine.sync;

import org.gradle.api.Project;
import org.gradle.api.Task;
import org.gradle.api.artifacts.Configuration;
import org.gradle.api.file.FileCollection;
import org.gradle.api.file.SourceDirectorySet;
import org.gradle.api.tasks.SourceSet;
import org.gradle.api.tasks.SourceSetContainer;

import java.io.File;
import java.util.ArrayList;
import java.util.Collection;
import java.util.Collections;
import java.util.IdentityHashMap;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

/** Reads the JVM source sets of one project. */
final class SourceSetReader {
    /** Languages whose source directory sets hang off a Gradle source set (besides resources). */
    static final String[] LANGUAGES = {"java", "scala", "kotlin", "groovy"};

    private final Project project;
    private final ClasspathResolver classpaths;
    private final List<String> errors;
    /** Output directory of every source set in the project, to name it on classpaths. */
    private final Map<String, String> outputOwners = new LinkedHashMap<>();

    SourceSetReader(Project project, ClasspathResolver classpaths, List<String> errors) {
        this.project = project;
        this.classpaths = classpaths;
        this.errors = errors;
    }

    static SourceSetContainer sourceSets(Project project) {
        // An extension since Gradle 7.1; a JavaPluginConvention property before that.
        Object value = project.getExtensions().findByName("sourceSets");
        if (!(value instanceof SourceSetContainer)) {
            value = project.findProperty("sourceSets");
        }
        return value instanceof SourceSetContainer ? (SourceSetContainer) value : null;
    }

    List<Object> read() {
        List<Object> out = new ArrayList<>();
        SourceSetContainer container = sourceSets(project);
        if (container == null) {
            return out;
        }
        List<SourceSet> all = new ArrayList<>(container);
        outputOwners.clear();
        for (SourceSet sourceSet : all) {
            for (File dir : sourceSet.getOutput().getClassesDirs().getFiles()) {
                outputOwners.put(dir.getAbsolutePath(), sourceSet.getName());
            }
            File resourcesDir = sourceSet.getOutput().getResourcesDir();
            if (resourcesDir != null) {
                outputOwners.put(resourcesDir.getAbsolutePath(), sourceSet.getName());
            }
        }
        for (SourceSet sourceSet : all) {
            try {
                out.add(read(sourceSet));
            } catch (Throwable e) {
                errors.add("sourceSet " + sourceSet.getName() + ": " + e);
            }
        }
        return out;
    }

    private Map<String, Object> read(SourceSet sourceSet) {
        Map<String, Object> out = new LinkedHashMap<>();
        out.put("name", sourceSet.getName());

        Map<String, Object> sources = new LinkedHashMap<>();
        Map<String, Object> compile = new LinkedHashMap<>();
        Roots roots = new Roots();
        List<Object> generated = new ArrayList<>();
        Set<String> annotationProcessorPath = new LinkedHashSet<>();

        for (String language : LANGUAGES) {
            SourceDirectorySet dirs = languageDirs(sourceSet, language);
            if (dirs == null) {
                continue;
            }
            String taskName = "java".equals(language)
                ? sourceSet.getCompileJavaTaskName()
                : sourceSet.getCompileTaskName(language);
            Task task = project.getTasks().findByName(taskName);
            Map<String, Object> set = directorySet(dirs);
            set.put("compileTask", task == null ? null : task.getPath());
            File destination = task == null ? null : destination(task);
            if (destination == null) {
                destination = Reflect.file(Reflect.call(dirs, "getDestinationDirectory"));
            }
            set.put("outputDir", destination == null ? null : destination.getAbsolutePath());
            sources.put(language, set);
            // A Scala or Groovy source set whose filter takes .java files compiles Java jointly.
            boolean jointJava = !"java".equals(language) && dirs.getFilter().getIncludes().contains("**/*.java");
            for (File dir : dirs.getSrcDirs()) {
                roots.add(dir, "source", language, taskName);
                if (jointJava) {
                    roots.add(dir, "source", "java", taskName);
                }
            }
            if (task != null) {
                Map<String, Object> taskInfo = compileTask(task);
                compile.put(language, taskInfo);
                File gen = generatedSourcesDir(task);
                if (gen != null) {
                    Map<String, Object> g = new LinkedHashMap<>();
                    g.put("dir", gen.getAbsolutePath());
                    g.put("kind", "annotationProcessor");
                    g.put("task", task.getPath());
                    g.put("exists", gen.isDirectory());
                    generated.add(g);
                    roots.add(gen, "generated", language, taskName);
                }
                annotationProcessorPath.addAll(annotationProcessorPath(task));
            }
        }

        SourceDirectorySet resources = sourceSet.getResources();
        Map<String, Object> resourceSet = directorySet(resources);
        File resourcesDir = sourceSet.getOutput().getResourcesDir();
        resourceSet.put("outputDir", resourcesDir == null ? null : resourcesDir.getAbsolutePath());
        sources.put("resources", resourceSet);
        for (File dir : resources.getSrcDirs()) {
            roots.add(dir, "resource", "resources", sourceSet.getProcessResourcesTaskName());
        }

        out.put("sources", sources);
        // Every directory whose .java files belong to this source set, whichever task compiles them
        // (for example src/main/java under a jointly compiled Scala source set).
        out.put("javaSourceDirs", Reflect.paths(sourceSet.getAllJava().getSrcDirs()));
        out.put("roots", roots.list());
        out.put("generatedSources", generated);

        Map<String, Object> output = new LinkedHashMap<>();
        output.put("classesDirs", Reflect.paths(sourceSet.getOutput().getClassesDirs().getFiles()));
        output.put("resourcesDir", resourcesDir == null ? null : resourcesDir.getAbsolutePath());
        out.put("output", output);

        out.put("compile", compile);
        out.put("annotationProcessorPath", new ArrayList<>(annotationProcessorPath));

        String compileName = sourceSet.getCompileClasspathConfigurationName();
        String runtimeName = sourceSet.getRuntimeClasspathConfigurationName();
        ClasspathResolver.Result compileClasspath = classpaths.resolve(configuration(compileName));
        ClasspathResolver.Result runtimeClasspath = classpaths.resolve(configuration(runtimeName));
        Map<String, Object> configurations = new LinkedHashMap<>();
        configurations.put("compileClasspath", compileName);
        configurations.put("runtimeClasspath", runtimeName);
        configurations.put("annotationProcessor", Reflect.string(Reflect.call(sourceSet, "getAnnotationProcessorConfigurationName")));
        out.put("configurations", configurations);
        out.put("compileClasspath", classpath(sourceSet.getCompileClasspath(), compileClasspath));
        out.put("runtimeClasspath", classpath(sourceSet.getRuntimeClasspath(), runtimeClasspath));
        List<Object> unresolved = new ArrayList<>();
        unresolved.addAll(compileClasspath.unresolved);
        unresolved.addAll(runtimeClasspath.unresolved);
        out.put("unresolved", unresolved);
        return out;
    }

    /**
     * The source set's classpath exactly as Gradle builds it, in order: the resolved configuration
     * entries, plus outputs of source sets (test sees main's classes; the runtime classpath holds
     * the source set's own output), plus any other files the build added.
     */
    private List<Map<String, Object>> classpath(FileCollection classpath, ClasspathResolver.Result resolved) {
        Set<File> files;
        try {
            files = classpath.getFiles();
        } catch (Throwable e) {
            // A strict resolution failure: the lenient configuration entries are the best we have.
            errors.add("classpath: " + e);
            return resolved.entries;
        }
        Map<String, Map<String, Object>> byFile = new LinkedHashMap<>();
        for (Map<String, Object> entry : resolved.entries) {
            if ("project".equals(entry.get("kind"))) {
                for (Object f : (List<?>) entry.get("files")) {
                    byFile.put(String.valueOf(f), entry);
                }
            } else {
                byFile.put(String.valueOf(entry.get("file")), entry);
            }
        }
        List<Map<String, Object>> out = new ArrayList<>();
        Set<Map<String, Object>> added = Collections.newSetFromMap(new IdentityHashMap<Map<String, Object>, Boolean>());
        Map<String, Object> lastOutput = null;
        for (File file : files) {
            String path = file.getAbsolutePath();
            Map<String, Object> entry = byFile.get(path);
            if (entry != null) {
                if (added.add(entry)) {
                    out.add(entry);
                }
                lastOutput = null;
                continue;
            }
            String owner = outputOwners.get(path);
            if (owner != null) {
                if (lastOutput == null || !owner.equals(lastOutput.get("sourceSet"))) {
                    lastOutput = new LinkedHashMap<>();
                    lastOutput.put("kind", "sourceSetOutput");
                    lastOutput.put("sourceSet", owner);
                    lastOutput.put("files", new ArrayList<String>());
                    out.add(lastOutput);
                }
                @SuppressWarnings("unchecked")
                List<String> outputFiles = (List<String>) lastOutput.get("files");
                outputFiles.add(path);
                continue;
            }
            lastOutput = null;
            Map<String, Object> other = new LinkedHashMap<>();
            other.put("kind", "file");
            other.put("file", path);
            out.add(other);
        }
        return out;
    }

    private Configuration configuration(String name) {
        return name == null ? null : project.getConfigurations().findByName(name);
    }

    /** {@code sourceSet.java} directly; other languages are extensions (Gradle 7.1+) or conventions (before). */
    static SourceDirectorySet languageDirs(SourceSet sourceSet, String language) {
        if ("java".equals(language)) {
            return sourceSet.getJava();
        }
        Object value = Reflect.property(sourceSet, language);
        if (value instanceof SourceDirectorySet) {
            return (SourceDirectorySet) value;
        }
        // Gradle before 7.1: ScalaSourceSet / GroovySourceSet conventions expose getScala() / getGroovy().
        String getter = "get" + Character.toUpperCase(language.charAt(0)) + language.substring(1);
        Object nested = Reflect.call(value, getter);
        return nested instanceof SourceDirectorySet ? (SourceDirectorySet) nested : null;
    }

    private static Map<String, Object> directorySet(SourceDirectorySet dirs) {
        Map<String, Object> out = new LinkedHashMap<>();
        out.put("srcDirs", Reflect.paths(dirs.getSrcDirs()));
        out.put("includes", new ArrayList<>(dirs.getIncludes()));
        out.put("excludes", new ArrayList<>(dirs.getExcludes()));
        // The filter carries the language's file patterns, for example **/*.scala and **/*.java.
        out.put("filter", new ArrayList<>(dirs.getFilter().getIncludes()));
        return out;
    }

    private static File destination(Task task) {
        File f = Reflect.file(Reflect.call(task, "getDestinationDirectory"));
        if (f == null) {
            f = Reflect.file(Reflect.call(task, "getDestinationDir"));
        }
        return f;
    }

    /** Compiler settings of a JavaCompile, ScalaCompile, GroovyCompile or Kotlin compile task. */
    private Map<String, Object> compileTask(Task task) {
        Map<String, Object> out = new LinkedHashMap<>();
        out.put("task", task.getPath());
        out.put("type", taskType(task));
        out.put("sourceCompatibility", Reflect.string(Reflect.call(task, "getSourceCompatibility")));
        out.put("targetCompatibility", Reflect.string(Reflect.call(task, "getTargetCompatibility")));
        Object options = Reflect.call(task, "getOptions");
        if (Reflect.isInstance(options, "org.gradle.api.tasks.compile.CompileOptions")) {
            Object release = Reflect.unwrap(Reflect.call(options, "getRelease"));
            out.put("release", release == null ? null : release.toString());
            out.put("encoding", Reflect.string(Reflect.call(options, "getEncoding")));
            Object all = Reflect.call(options, "getAllCompilerArgs");
            out.put("compilerArgs", Reflect.strings(all != null ? all : Reflect.call(options, "getCompilerArgs")));
        }
        Object scalaOptions = Reflect.call(task, "getScalaCompileOptions");
        if (scalaOptions != null) {
            out.put("scalaParameters", Reflect.strings(Reflect.call(scalaOptions, "getAdditionalParameters")));
        }
        Object toolchain = javaToolchain(task);
        if (toolchain != null) {
            out.put("toolchain", toolchain);
        }
        return out;
    }

    /** The JDK a compile task will use (Gradle 6.7+ javaCompiler, 7.2+ javaLauncher for Scala). */
    private Map<String, Object> javaToolchain(Task task) {
        try {
            Object tool = Reflect.unwrap(Reflect.call(task, "getJavaCompiler"));
            if (tool == null) {
                tool = Reflect.unwrap(Reflect.call(task, "getJavaLauncher"));
            }
            Object metadata = Reflect.call(tool, "getMetadata");
            if (metadata == null) {
                return null;
            }
            Map<String, Object> out = new LinkedHashMap<>();
            out.put("languageVersion", Reflect.string(Reflect.call(metadata, "getLanguageVersion")));
            out.put("vendor", Reflect.string(Reflect.call(metadata, "getVendor")));
            File home = Reflect.file(Reflect.call(metadata, "getInstallationPath"));
            out.put("javaHome", home == null ? null : home.getAbsolutePath());
            return out;
        } catch (Throwable e) {
            errors.add("toolchain of " + task.getPath() + ": " + e);
            return null;
        }
    }

    /** Annotation processor output: generatedSourceOutputDirectory (6.3+) or the older property. */
    private static File generatedSourcesDir(Task task) {
        Object options = Reflect.call(task, "getOptions");
        File f = Reflect.file(Reflect.call(options, "getGeneratedSourceOutputDirectory"));
        if (f == null) {
            f = Reflect.file(Reflect.call(options, "getAnnotationProcessorGeneratedSourcesDirectory"));
        }
        return f;
    }

    private List<String> annotationProcessorPath(Task task) {
        Object path = Reflect.call(Reflect.call(task, "getOptions"), "getAnnotationProcessorPath");
        if (!(path instanceof FileCollection)) {
            return new ArrayList<>();
        }
        try {
            return Reflect.paths(((FileCollection) path).getFiles());
        } catch (Throwable e) {
            errors.add("annotation processor path of " + task.getPath() + ": " + e);
            return new ArrayList<>();
        }
    }

    static String taskType(Task task) {
        return Reflect.declaredType(task).getName();
    }

    /** A flat list of content roots, merging a directory used by several languages. */
    private static final class Roots {
        private final Map<String, Map<String, Object>> byKey = new LinkedHashMap<>();

        @SuppressWarnings("unchecked")
        void add(File dir, String kind, String language, String task) {
            String path = dir.getAbsolutePath();
            String key = kind + "|" + path;
            Map<String, Object> root = byKey.get(key);
            if (root == null) {
                root = new LinkedHashMap<>();
                root.put("dir", path);
                root.put("kind", kind);
                root.put("languages", new ArrayList<String>());
                root.put("compiledBy", new ArrayList<String>());
                root.put("exists", dir.isDirectory());
                byKey.put(key, root);
            }
            addOnce((Collection<String>) root.get("languages"), language);
            addOnce((Collection<String>) root.get("compiledBy"), task);
        }

        private static void addOnce(Collection<String> list, String value) {
            if (value != null && !list.contains(value)) {
                list.add(value);
            }
        }

        List<Object> list() {
            return new ArrayList<Object>(byKey.values());
        }
    }
}
