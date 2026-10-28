package com.snoutcode.engine.jdtls;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

import java.io.File;
import java.io.IOException;
import java.io.Reader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;

/**
 * What to create in the JDT workspace for one model written by {@code snout-engine sync}: one Java
 * project per Gradle project that has Java sources. It uses no Eclipse API, so it can be tested on
 * its own.
 *
 * <p>Each Gradle project becomes one JDT project, the way Buildship maps them: the source sets
 * share it, and test source sets, and the libraries only they use, carry JDT's {@code test}
 * attribute so main code cannot see them.
 */
public final class ImportPlan {
    /** The Gradle root build directory the model describes. */
    public final File rootDir;
    /** Every project directory of every build in the model, whether or not it has Java sources. */
    public final List<File> projectDirs = new ArrayList<>();
    public final List<Project> projects = new ArrayList<>();
    /** Set when the sync failed and no model was available. */
    public String syncError;

    /** One JDT project. */
    public static final class Project {
        /** The workspace project name, unique in the plan. */
        public String name;
        public String buildName;
        public String gradlePath;
        public File projectDir;
        public final List<SourceRoot> sources = new ArrayList<>();
        /** Libraries and project references, in Gradle's classpath order. */
        public final List<Dependency> dependencies = new ArrayList<>();
        public final JavaSettings java = new JavaSettings();
    }

    /** A source directory, linked into the workspace project as a folder named {@link #link}. */
    public static final class SourceRoot {
        public File dir;
        public String link;
        public String sourceSet;
        public boolean test;
        public boolean generated;
        /** Exclusion patterns, relative to {@link #dir}, for source roots nested inside this one. */
        public final List<String> exclusions = new ArrayList<>();
    }

    /** A classpath entry: a jar or class folder, or another project of the plan. */
    public static final class Dependency {
        public File file;
        public File sources;
        /** Set for a reference to another project of the plan instead of a file. */
        public String project;
        public boolean test;
        /** Re-exported to projects that depend on this one (the Scala class folders). */
        public boolean exported;

        @Override
        public String toString() {
            return (project != null ? "project " + project : String.valueOf(file)) + (test ? " (test)" : "");
        }
    }

    /** Compiler settings, from the main source set's Java compile task. */
    public static final class JavaSettings {
        public String javaHome;
        public String source;
        public String target;
        public String release;
        public String encoding;
        public boolean parameters;
        public boolean preview;
    }

    private ImportPlan(File rootDir) {
        this.rootDir = rootDir;
    }

    public static ImportPlan read(Path modelFile) throws IOException {
        try (Reader reader = Files.newBufferedReader(modelFile, StandardCharsets.UTF_8)) {
            return from(JsonParser.parseReader(reader).getAsJsonObject());
        }
    }

    public static ImportPlan from(JsonObject model) {
        ImportPlan plan = new ImportPlan(new File(string(model, "rootDir")));
        plan.syncError = string(model, "syncError");
        String gradleJavaHome = string(model, "gradleJavaHome");
        JsonArray builds = array(model, "builds");

        // Name every project first, so classpath entries can refer to any of them.
        Map<String, Project> byBuildAndPath = new HashMap<>();
        Map<String, Project> byOutputDir = new HashMap<>();
        Map<JsonObject, Project> byModel = new LinkedHashMap<>();
        Set<String> names = new LinkedHashSet<>();
        for (JsonElement b : builds) {
            JsonObject build = b.getAsJsonObject();
            String buildName = string(build, "name");
            for (JsonElement p : array(build, "projects")) {
                JsonObject project = p.getAsJsonObject();
                File projectDir = new File(string(project, "projectDir"));
                plan.projectDirs.add(projectDir);
                if (!hasJavaSources(project)) {
                    continue;
                }
                Project out = new Project();
                out.buildName = buildName;
                out.gradlePath = string(project, "path");
                out.projectDir = projectDir;
                out.name = uniqueName(names, string(project, "name"), buildName, out.gradlePath);
                byModel.put(project, out);
                for (String key : buildKeys(build, buildName)) {
                    byBuildAndPath.putIfAbsent(key + "|" + out.gradlePath, out);
                }
                for (JsonElement s : array(project, "sourceSets")) {
                    JsonObject output = object(s.getAsJsonObject(), "output");
                    for (String dir : strings(output, "classesDirs")) {
                        byOutputDir.put(dir, out);
                    }
                    String resources = string(output, "resourcesDir");
                    if (resources != null) {
                        byOutputDir.put(resources, out);
                    }
                }
            }
        }

        for (Map.Entry<JsonObject, Project> e : byModel.entrySet()) {
            JsonObject project = e.getKey();
            Project out = e.getValue();
            List<JsonObject> sourceSets = sourceSetsMainFirst(project);
            Set<String> testSets = testSourceSets(project);
            addSources(out, sourceSets, testSets);
            addDependencies(out, sourceSets, testSets, byBuildAndPath, byOutputDir);
            readJavaSettings(out, project, sourceSets, gradleJavaHome);
            plan.projects.add(out);
        }
        return plan;
    }

    // ---- sources

    private static boolean hasJavaSources(JsonObject project) {
        for (JsonElement s : array(project, "sourceSets")) {
            if (!strings(s.getAsJsonObject(), "javaSourceDirs").isEmpty()) {
                return true;
            }
        }
        return false;
    }

    private static void addSources(Project out, List<JsonObject> sourceSets, Set<String> testSets) {
        Set<String> seen = new LinkedHashSet<>();
        Set<String> links = new LinkedHashSet<>();
        links.add(WorkspaceLayout.OUTPUT_FOLDER);
        for (JsonObject sourceSet : sourceSets) {
            String name = string(sourceSet, "name");
            boolean test = testSets.contains(name);
            List<String> dirs = new ArrayList<>(strings(sourceSet, "javaSourceDirs"));
            List<String> generated = new ArrayList<>();
            for (JsonElement r : array(sourceSet, "roots")) {
                JsonObject root = r.getAsJsonObject();
                if ("generated".equals(string(root, "kind")) && strings(root, "languages").contains("java")) {
                    generated.add(string(root, "dir"));
                }
            }
            for (String dir : concat(dirs, generated)) {
                File file = new File(dir);
                // A missing folder would be a classpath error in JDT; the next import picks it up.
                if (!file.isDirectory() || !seen.add(normalize(file))) {
                    continue;
                }
                SourceRoot root = new SourceRoot();
                root.dir = file;
                root.sourceSet = name;
                root.test = test;
                root.generated = generated.contains(dir) && !dirs.contains(dir);
                root.link = uniqueLink(links, linkName(out.projectDir, file));
                out.sources.add(root);
            }
        }
        // Linked folders are separate resources, so JDT cannot see that one root lies inside
        // another. Exclude the inner root from the outer one, or its files would load twice.
        for (SourceRoot outer : out.sources) {
            Path outerPath = outer.dir.toPath().toAbsolutePath().normalize();
            for (SourceRoot inner : out.sources) {
                Path innerPath = inner.dir.toPath().toAbsolutePath().normalize();
                if (inner != outer && innerPath.startsWith(outerPath) && !innerPath.equals(outerPath)) {
                    outer.exclusions.add(outerPath.relativize(innerPath).toString().replace(File.separatorChar, '/') + "/");
                }
            }
        }
    }

    /**
     * A readable folder name for a linked source root: its path relative to the project
     * directory, or for a folder outside it, the path from the nearest common ancestor.
     * {@code src/main/java} becomes {@code src-main-java}; {@code ../core/src/main/java} becomes
     * {@code core-src-main-java}.
     */
    static String linkName(File projectDir, File dir) {
        Path base = projectDir.toPath().toAbsolutePath().normalize();
        Path target = dir.toPath().toAbsolutePath().normalize();
        Path relative;
        if (target.startsWith(base)) {
            relative = base.relativize(target);
        } else {
            Path common = base;
            while (common != null && !target.startsWith(common)) {
                common = common.getParent();
            }
            relative = common == null ? target : common.relativize(target);
        }
        StringBuilder name = new StringBuilder();
        for (Path segment : relative) {
            String s = segment.toString();
            if (s.isEmpty() || s.equals("..") || s.equals(".")) {
                continue;
            }
            if (name.length() > 0) {
                name.append('-');
            }
            name.append(s.replaceAll("[\\\\/:*?\"<>|]", "_"));
        }
        return name.length() == 0 ? "src" : name.toString();
    }

    private static String uniqueLink(Set<String> taken, String name) {
        String candidate = name;
        for (int i = 2; taken.contains(candidate.toLowerCase(Locale.ROOT)) || candidate.startsWith("."); i++) {
            candidate = name.replaceFirst("^\\.+", "") + "-" + i;
        }
        taken.add(candidate.toLowerCase(Locale.ROOT));
        return candidate;
    }

    // ---- classpath

    private static void addDependencies(Project out, List<JsonObject> sourceSets, Set<String> testSets,
                                        Map<String, Project> byBuildAndPath, Map<String, Project> byOutputDir) {
        Map<String, Dependency> byKey = new LinkedHashMap<>();
        Set<String> usedByMain = new LinkedHashSet<>();
        for (JsonObject sourceSet : sourceSets) {
            String name = string(sourceSet, "name");
            boolean test = testSets.contains(name);

            // The Scala bridge: classes compileScala produced, for Java code that uses Scala types,
            // with the Scala sources attached so navigation shows them instead of decompiling.
            for (JsonElement f : array(object(sourceSet, "scalaBridge"), "folders")) {
                JsonObject folder = f.getAsJsonObject();
                String classes = string(folder, "classesDir");
                if (classes == null || !new File(classes).isDirectory()) {
                    continue;
                }
                Dependency d = new Dependency();
                d.file = new File(classes);
                String sources = string(folder, "sourceDir");
                d.sources = sources == null ? null : new File(sources);
                d.exported = true;
                add(byKey, usedByMain, "file:" + normalize(d.file), d, test);
            }

            for (JsonElement e : array(sourceSet, "compileClasspath")) {
                JsonObject entry = e.getAsJsonObject();
                String kind = string(entry, "kind");
                if ("sourceSetOutput".equals(kind)) {
                    continue; // another source set of this same project
                }
                if ("project".equals(kind)) {
                    Project target = byBuildAndPath.get(string(entry, "build") + "|" + string(entry, "path"));
                    if (target == null) {
                        target = projectOwning(strings(entry, "files"), byOutputDir);
                    }
                    if (target != null) {
                        if (target != out) {
                            Dependency d = new Dependency();
                            d.project = target.name;
                            add(byKey, usedByMain, "project:" + target.name, d, test);
                        }
                    } else {
                        for (String f : strings(entry, "files")) {
                            addFile(byKey, usedByMain, new File(f), null, test);
                        }
                    }
                    continue;
                }
                String file = string(entry, "file");
                if (file == null) {
                    continue;
                }
                Project owner = byOutputDir.get(file);
                if (owner != null) {
                    if (owner != out) {
                        Dependency d = new Dependency();
                        d.project = owner.name;
                        add(byKey, usedByMain, "project:" + owner.name, d, test);
                    }
                    continue;
                }
                String sources = string(entry, "sources");
                addFile(byKey, usedByMain, new File(file), sources == null ? null : new File(sources), test);
            }
        }
        for (Map.Entry<String, Dependency> e : byKey.entrySet()) {
            e.getValue().test = !usedByMain.contains(e.getKey());
            out.dependencies.add(e.getValue());
        }
    }

    private static void addFile(Map<String, Dependency> byKey, Set<String> usedByMain, File file, File sources, boolean test) {
        if (!file.exists()) {
            return; // JDT would report a missing library as an error on the project
        }
        Dependency d = new Dependency();
        d.file = file;
        d.sources = sources != null && sources.exists() ? sources : null;
        add(byKey, usedByMain, "file:" + normalize(file), d, test);
    }

    private static void add(Map<String, Dependency> byKey, Set<String> usedByMain, String key, Dependency d, boolean test) {
        Dependency existing = byKey.putIfAbsent(key, d);
        if (existing != null && existing.sources == null) {
            existing.sources = d.sources;
        }
        if (!test) {
            usedByMain.add(key);
        }
    }

    private static Project projectOwning(List<String> files, Map<String, Project> byOutputDir) {
        for (String f : files) {
            Project p = byOutputDir.get(f);
            if (p != null) {
                return p;
            }
        }
        return null;
    }

    // ---- compiler settings

    private static void readJavaSettings(Project out, JsonObject project, List<JsonObject> sourceSets, String gradleJavaHome) {
        JavaSettings java = out.java;
        JsonObject projectJava = object(project, "java");
        java.source = string(projectJava, "sourceCompatibility");
        java.target = string(projectJava, "targetCompatibility");
        for (JsonObject sourceSet : sourceSets) {
            JsonObject compile = object(object(sourceSet, "compile"), "java");
            if (compile == null) {
                continue;
            }
            java.source = firstNonNull(string(compile, "sourceCompatibility"), java.source);
            java.target = firstNonNull(string(compile, "targetCompatibility"), java.target);
            java.release = string(compile, "release");
            java.encoding = string(compile, "encoding");
            List<String> args = strings(compile, "compilerArgs");
            java.parameters = args.contains("-parameters");
            java.preview = args.contains("--enable-preview");
            int release = args.indexOf("--release");
            if (java.release == null && release >= 0 && release + 1 < args.size()) {
                java.release = args.get(release + 1);
            }
            java.javaHome = string(object(compile, "toolchain"), "javaHome");
            break;
        }
        if (java.javaHome == null) {
            java.javaHome = gradleJavaHome;
        }
    }

    // ---- source sets

    private static List<JsonObject> sourceSetsMainFirst(JsonObject project) {
        List<JsonObject> out = new ArrayList<>();
        for (JsonElement s : array(project, "sourceSets")) {
            JsonObject sourceSet = s.getAsJsonObject();
            if ("main".equals(string(sourceSet, "name"))) {
                out.add(0, sourceSet);
            } else {
                out.add(sourceSet);
            }
        }
        return out;
    }

    /** Source sets that a test task runs, plus {@code test} and names ending in Test or Tests. */
    private static Set<String> testSourceSets(JsonObject project) {
        Set<String> out = new LinkedHashSet<>();
        for (JsonElement t : array(project, "testTasks")) {
            out.addAll(strings(t.getAsJsonObject(), "sourceSets"));
        }
        for (JsonElement s : array(project, "sourceSets")) {
            String name = string(s.getAsJsonObject(), "name");
            if (name != null && (name.equals("test") || name.endsWith("Test") || name.endsWith("Tests"))) {
                out.add(name);
            }
        }
        out.remove("main");
        return out;
    }

    // ---- names

    private static String uniqueName(Set<String> taken, String name, String buildName, String path) {
        String candidate = sanitize(name == null || name.isEmpty() ? "project" : name);
        if (taken.contains(candidate.toLowerCase(Locale.ROOT)) && buildName != null) {
            candidate = sanitize(buildName + (":".equals(path) ? "" : path.replace(':', '-')));
        }
        String base = candidate;
        for (int i = 2; taken.contains(candidate.toLowerCase(Locale.ROOT)); i++) {
            candidate = base + "-" + i;
        }
        taken.add(candidate.toLowerCase(Locale.ROOT));
        return candidate;
    }

    private static String sanitize(String name) {
        String s = name.replaceAll("[\\\\/:*?\"<>|]", "_").trim();
        return s.startsWith(".") ? "_" + s.substring(1) : s;
    }

    /**
     * The keys a project dependency may name its build by: the build path on Gradle 8.2+ ({@code :}
     * for the root build, {@code :tools} for an included one), the build name before.
     */
    private static List<String> buildKeys(JsonObject build, String buildName) {
        List<String> keys = new ArrayList<>();
        if ("root".equals(string(build, "kind"))) {
            keys.add(":");
            return keys;
        }
        String dirName = new File(String.valueOf(string(build, "rootDir"))).getName();
        for (String name : new String[] {dirName, buildName}) {
            if (name != null && !name.isEmpty()) {
                keys.add(":" + name);
                keys.add(name);
            }
        }
        return keys;
    }

    // ---- JSON helpers

    static String normalize(File file) {
        return file.toPath().toAbsolutePath().normalize().toString();
    }

    private static <T> T firstNonNull(T a, T b) {
        return a != null ? a : b;
    }

    private static List<String> concat(List<String> a, List<String> b) {
        List<String> out = new ArrayList<>(a);
        out.addAll(b);
        return out;
    }

    static String string(JsonObject o, String key) {
        if (o == null) {
            return null;
        }
        JsonElement e = o.get(key);
        return e == null || e.isJsonNull() ? null : e.getAsString();
    }

    static JsonObject object(JsonObject o, String key) {
        if (o == null) {
            return null;
        }
        JsonElement e = o.get(key);
        return e != null && e.isJsonObject() ? e.getAsJsonObject() : null;
    }

    static JsonArray array(JsonObject o, String key) {
        if (o == null) {
            return new JsonArray();
        }
        JsonElement e = o.get(key);
        return e != null && e.isJsonArray() ? e.getAsJsonArray() : new JsonArray();
    }

    static List<String> strings(JsonObject o, String key) {
        List<String> out = new ArrayList<>();
        for (JsonElement e : array(o, key)) {
            if (!e.isJsonNull()) {
                out.add(e.getAsString());
            }
        }
        return out;
    }
}
