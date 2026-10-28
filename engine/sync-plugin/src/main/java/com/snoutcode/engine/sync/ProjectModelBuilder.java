package com.snoutcode.engine.sync;

import com.snoutcode.engine.sync.model.SnoutProjectModel;
import org.gradle.api.Plugin;
import org.gradle.api.Project;
import org.gradle.api.initialization.IncludedBuild;
import org.gradle.api.invocation.Gradle;
import org.gradle.tooling.provider.model.ToolingModelBuilder;
import org.gradle.util.GradleVersion;

import java.io.File;
import java.util.ArrayList;
import java.util.Collection;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;

/**
 * Builds {@link SnoutProjectModel} for one project. Each section is read independently; a failure
 * is recorded under {@code errors} instead of failing the whole sync.
 */
public final class ProjectModelBuilder implements ToolingModelBuilder {
    /** Model format version; bump on incompatible JSON changes. */
    public static final int MODEL_VERSION = 1;

    /** System property ({@code -Dsnout.sync.sources=false}) that turns off source jar lookup. */
    public static final String SOURCES_PROPERTY = "snout.sync.sources";

    @Override
    public boolean canBuild(String modelName) {
        return SnoutProjectModel.class.getName().equals(modelName);
    }

    @Override
    public Object buildAll(String modelName, Project project) {
        return new DefaultSnoutProjectModel(project.getPath(), Json.write(build(project)));
    }

    static Map<String, Object> build(Project project) {
        long start = System.nanoTime();
        List<String> errors = new ArrayList<>();
        Map<String, Object> out = new LinkedHashMap<>();
        out.put("modelVersion", MODEL_VERSION);
        out.put("path", project.getPath());
        out.put("name", project.getName());
        out.put("projectDir", project.getProjectDir().getAbsolutePath());
        out.put("buildFile", project.getBuildFile().isFile() ? project.getBuildFile().getAbsolutePath() : null);
        out.put("buildDir", buildDir(project));
        out.put("group", String.valueOf(project.getGroup()));
        out.put("version", String.valueOf(project.getVersion()));
        out.put("description", project.getDescription());

        if (project == project.getRootProject()) {
            section(out, errors, "build", () -> buildInfo(project));
        }
        section(out, errors, "plugins", () -> plugins(project));
        section(out, errors, "appliedScripts", () -> appliedScripts(project));
        section(out, errors, "java", () -> java(project));

        ClasspathResolver classpaths = new ClasspathResolver(project);
        section(out, errors, "sourceSets", () -> new SourceSetReader(project, classpaths, errors).read());
        if (!"false".equals(System.getProperty(SOURCES_PROPERTY))) {
            section(out, errors, "sourceJarsFound", classpaths::attachSources);
        }
        section(out, errors, "testTasks", () -> TestTaskReader.read(project, errors));
        section(out, errors, "springBoot", () -> SpringBootReader.read(project, errors));
        section(out, errors, "application", () -> application(project));

        out.put("errors", errors);
        out.put("buildMillis", (System.nanoTime() - start) / 1_000_000);
        return out;
    }

    interface Section {
        Object read() throws Exception;
    }

    private static void section(Map<String, Object> out, List<String> errors, String key, Section section) {
        try {
            out.put(key, section.read());
        } catch (Throwable e) {
            out.put(key, null);
            errors.add(key + ": " + e);
        }
    }

    private static String buildDir(Project project) {
        File dir = Reflect.file(Reflect.call(Reflect.call(project, "getLayout"), "getBuildDirectory"));
        if (dir == null) {
            dir = Reflect.file(Reflect.call(project, "getBuildDir"));
        }
        return dir == null ? null : dir.getAbsolutePath();
    }

    /** Build-wide facts, reported on the root project of each build. */
    private static Map<String, Object> buildInfo(Project root) {
        Gradle gradle = root.getGradle();
        Map<String, Object> out = new LinkedHashMap<>();
        File rootDir = root.getRootDir();
        out.put("rootDir", rootDir.getAbsolutePath());
        out.put("gradleVersion", GradleVersion.current().getVersion());
        Map<String, Object> jvm = new LinkedHashMap<>();
        jvm.put("javaHome", System.getProperty("java.home"));
        jvm.put("javaVersion", System.getProperty("java.version"));
        out.put("gradleJvm", jvm);
        out.put("settingsFile", existing(new File(rootDir, "settings.gradle"), new File(rootDir, "settings.gradle.kts")));

        List<Object> included = new ArrayList<>();
        for (IncludedBuild build : gradle.getIncludedBuilds()) {
            Map<String, Object> b = new LinkedHashMap<>();
            b.put("name", build.getName());
            b.put("projectDir", build.getProjectDir().getAbsolutePath());
            included.add(b);
        }
        out.put("includedBuilds", included);
        File buildSrc = new File(rootDir, "buildSrc");
        out.put("buildSrc", buildSrc.isDirectory() ? buildSrc.getAbsolutePath() : null);

        // Files whose change should trigger a resync.
        Set<String> triggers = new LinkedHashSet<>();
        ScriptTracker tracker = ScriptTracker.of(gradle);
        if (tracker != null) {
            for (String script : tracker.allScripts()) {
                if (!isInitScript(script)) {
                    triggers.add(script);
                }
            }
        }
        addIfExists(triggers, out.get("settingsFile") == null ? null : new File((String) out.get("settingsFile")));
        for (Project p : root.getAllprojects()) {
            addIfExists(triggers, p.getBuildFile());
            addIfExists(triggers, new File(p.getProjectDir(), "gradle.properties"));
            for (String script : ScriptTracker.scanBuildFile(p.getBuildFile())) {
                triggers.add(script);
            }
        }
        addIfExists(triggers, new File(rootDir, "gradle/libs.versions.toml"));
        addIfExists(triggers, new File(rootDir, "gradle/wrapper/gradle-wrapper.properties"));
        if (buildSrc.isDirectory()) {
            triggers.add(buildSrc.getAbsolutePath() + File.separator);
        }
        out.put("resyncTriggers", new ArrayList<>(triggers));
        out.put("scriptTracking", tracker != null && tracker.isListening() ? "listener" : "buildFileScan");
        return out;
    }

    private static boolean isInitScript(String path) {
        return new File(path).getName().startsWith("snout-sync-init");
    }

    private static String existing(File... files) {
        for (File f : files) {
            if (f.isFile()) {
                return f.getAbsolutePath();
            }
        }
        return null;
    }

    private static void addIfExists(Collection<String> out, File file) {
        if (file != null && file.isFile()) {
            out.add(file.getAbsolutePath());
        }
    }

    /** Plugin ids (where Gradle knows them) and implementation classes, in application order. */
    @SuppressWarnings("rawtypes") // PluginContainer is a collection of raw Plugin
    private static List<Object> plugins(Project project) {
        List<Object> out = new ArrayList<>();
        Set<String> seen = new LinkedHashSet<>();
        for (Plugin<?> plugin : new ArrayList<Plugin>(project.getPlugins())) {
            Class<?> type = Reflect.declaredType(plugin);
            if (!seen.add(type.getName())) {
                continue;
            }
            Map<String, Object> p = new LinkedHashMap<>();
            p.put("id", pluginId(project, type));
            p.put("class", type.getName());
            out.add(p);
        }
        return out;
    }

    /** PluginManagerInternal.findPluginIdForClass, an internal API, so reflective and optional. */
    private static String pluginId(Project project, Class<?> type) {
        Object id = Reflect.call(project.getPluginManager(), "findPluginIdForClass", Class.class, type);
        if (id instanceof Optional) {
            id = ((Optional<?>) id).orElse(null);
        }
        if (id == null) {
            return null;
        }
        Object text = Reflect.call(id, "getId");
        return text == null ? id.toString() : text.toString();
    }

    /** Scripts applied with {@code apply from}, in order, excluding the project's own build file. */
    private static Map<String, Object> appliedScripts(Project project) {
        Map<String, Object> out = new LinkedHashMap<>();
        ScriptTracker tracker = ScriptTracker.of(project.getGradle());
        String buildFile = project.getBuildFile().getAbsolutePath();
        List<String> scripts = new ArrayList<>();
        String source;
        if (tracker != null && tracker.isListening() && !tracker.scriptsOf(project.getPath()).isEmpty()) {
            source = "listener";
            for (String s : tracker.scriptsOf(project.getPath())) {
                if (!s.equals(buildFile)) {
                    scripts.add(s);
                }
            }
        } else {
            source = "buildFileScan";
            scripts.addAll(ScriptTracker.scanBuildFile(project.getBuildFile()));
        }
        out.put("scripts", scripts);
        out.put("source", source);
        return out;
    }

    /**
     * The {@code application} plugin's main class and run task. {@code mainClass} is a property
     * from Gradle 6.4; before that, {@code mainClassName} on the project convention.
     */
    private static Map<String, Object> application(Project project) {
        if (!project.getPluginManager().hasPlugin("application")) {
            return null;
        }
        Object extension = project.getExtensions().findByName("application");
        Object mainClass = Reflect.unwrap(Reflect.property(extension, "mainClass"));
        if (mainClass == null) {
            mainClass = Reflect.unwrap(project.findProperty("mainClassName"));
        }
        Object module = Reflect.unwrap(Reflect.property(extension, "mainModule"));
        Map<String, Object> out = new LinkedHashMap<>();
        out.put("mainClass", mainClass == null ? null : mainClass.toString());
        out.put("mainModule", module == null ? null : module.toString());
        out.put("applicationName", Reflect.string(Reflect.property(extension, "applicationName")));
        out.put("jvmArgs", Reflect.strings(Reflect.property(extension, "applicationDefaultJvmArgs")));
        org.gradle.api.Task run = project.getTasks().findByName("run");
        out.put("runTask", run == null ? null : run.getPath());
        return out;
    }

    /** Project-wide Java settings: compatibility levels and the toolchain spec, if one is set. */
    private static Map<String, Object> java(Project project) {
        Object extension = project.getExtensions().findByName("java");
        if (extension == null && project.findProperty("sourceCompatibility") == null) {
            return null;
        }
        Map<String, Object> out = new LinkedHashMap<>();
        Object source = Reflect.call(extension, "getSourceCompatibility");
        Object target = Reflect.call(extension, "getTargetCompatibility");
        if (source == null) {
            source = project.findProperty("sourceCompatibility");
            target = project.findProperty("targetCompatibility");
        }
        out.put("sourceCompatibility", Reflect.string(source));
        out.put("targetCompatibility", Reflect.string(target));
        Object toolchain = Reflect.call(extension, "getToolchain");
        Map<String, Object> spec = null;
        if (toolchain != null) {
            Object languageVersion = Reflect.unwrap(Reflect.call(toolchain, "getLanguageVersion"));
            if (languageVersion != null) {
                spec = new LinkedHashMap<>();
                spec.put("languageVersion", languageVersion.toString());
                spec.put("vendor", Reflect.string(Reflect.call(toolchain, "getVendor")));
                spec.put("implementation", Reflect.string(Reflect.call(toolchain, "getImplementation")));
            }
        }
        out.put("toolchain", spec);
        return out;
    }
}
