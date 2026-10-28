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
 * project per Gradle project that has Java sources, across every Gradle build of the folder. It
 * uses no Eclipse API, so it can be tested on its own.
 *
 * <p>Each Gradle project becomes one JDT project, the way Buildship maps them: the source sets
 * share it, and test source sets, and the libraries only they use, carry JDT's {@code test}
 * attribute so main code cannot see them.
 *
 * <p>A source folder that several Gradle projects compile is owned by one JDT project only, so
 * that search, references and navigation see each file once (see {@link #shareDuplicateRoots}).
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
        /** The module id in the model ({@code shop}, {@code shop:app}, {@code shop/tools:lib}); null for a shared project. */
        public String moduleId;
        public File projectDir;
        /** Resource folders of the main source sets, and of the test source sets. */
        public final List<File> resourceDirs = new ArrayList<>();
        public final List<File> testResourceDirs = new ArrayList<>();
        public final List<SourceRoot> sources = new ArrayList<>();
        /** Libraries and project references, in Gradle's classpath order. */
        public final List<Dependency> dependencies = new ArrayList<>();
        public final JavaSettings java = new JavaSettings();
        /**
         * For a project made for shared source folders, the projects that compile them; empty for
         * a project made from a Gradle project.
         */
        public final List<Project> sharedBy = new ArrayList<>();
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
        /** {@code group:name} of a module dependency, to merge classpaths by; null for a file. */
        public String coordinates;
        public String version;

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
        // Model version 2 holds many Gradle builds; version 1 was the model of one.
        JsonArray gradleBuilds = model.has("gradleBuilds") ? array(model, "gradleBuilds") : legacy(model);

        // Name every project first, so classpath entries can refer to any of them.
        Map<String, Project> byBuildAndPath = new HashMap<>();
        Map<String, Project> byOutputDir = new HashMap<>();
        Map<JsonObject, Project> byModel = new LinkedHashMap<>();
        Map<JsonObject, String> gradleBuildOf = new HashMap<>();
        Map<JsonObject, String> javaHomeOf = new HashMap<>();
        Set<String> names = new LinkedHashSet<>();
        Set<String> projectDirs = new LinkedHashSet<>();
        for (int g = 0; g < gradleBuilds.size(); g++) {
            JsonObject gradleBuild = gradleBuilds.get(g).getAsJsonObject();
            String gradleRoot = string(gradleBuild, "rootDir");
            if (gradleRoot != null && projectDirs.add(normalize(new File(gradleRoot)))) {
                plan.projectDirs.add(new File(gradleRoot)); // a failed build is claimed too
            }
            for (JsonElement b : array(gradleBuild, "builds")) {
                JsonObject build = b.getAsJsonObject();
                String buildName = string(build, "name");
                for (JsonElement p : array(build, "projects")) {
                    JsonObject project = p.getAsJsonObject();
                    File projectDir = new File(string(project, "projectDir"));
                    if (projectDirs.add(normalize(projectDir))) {
                        plan.projectDirs.add(projectDir);
                    }
                    if (!hasJavaSources(project)) {
                        continue;
                    }
                    Project out = new Project();
                    out.buildName = buildName;
                    out.gradlePath = string(project, "path");
                    out.projectDir = projectDir;
                    out.name = uniqueName(names, string(project, "name"), buildName, out.gradlePath);
                    String buildId = string(gradleBuild, "id");
                    if (buildId != null) {
                        String prefix = "root".equals(string(build, "kind")) ? buildId : buildId + "/" + buildName;
                        out.moduleId = ":".equals(out.gradlePath) || out.gradlePath == null ? prefix : prefix + out.gradlePath;
                    }
                    byModel.put(project, out);
                    gradleBuildOf.put(project, String.valueOf(g));
                    javaHomeOf.put(project, string(gradleBuild, "gradleJavaHome"));
                    for (String key : buildKeys(build, buildName)) {
                        byBuildAndPath.putIfAbsent(g + "|" + key + "|" + out.gradlePath, out);
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
        }

        for (Map.Entry<JsonObject, Project> e : byModel.entrySet()) {
            JsonObject project = e.getKey();
            Project out = e.getValue();
            List<JsonObject> sourceSets = sourceSetsMainFirst(project);
            Set<String> testSets = testSourceSets(project);
            addSources(out, sourceSets, testSets);
            addResources(out, sourceSets, testSets);
            addDependencies(out, sourceSets, testSets, gradleBuildOf.get(project), byBuildAndPath, byOutputDir);
            readJavaSettings(out, project, sourceSets, javaHomeOf.get(project));
            plan.projects.add(out);
        }
        shareDuplicateRoots(plan, names);
        return plan;
    }

    /** A version 1 model, of one Gradle build, as the one entry of a version 2 model's builds. */
    private static JsonArray legacy(JsonObject model) {
        JsonObject entry = new JsonObject();
        entry.add("rootDir", model.get("rootDir"));
        entry.add("gradleJavaHome", model.get("gradleJavaHome"));
        entry.add("builds", array(model, "builds"));
        JsonArray out = new JsonArray();
        out.add(entry);
        return out;
    }

    // ---- source folders that several projects compile

    /**
     * Gives every source folder that more than one project compiles a single owner, because JDT
     * cannot sensibly hold the same files in several projects: each copy is indexed, searched and
     * compiled again, so references list every use once per copy and navigation picks one at
     * random.
     *
     * <ul>
     *   <li>When the folder lies in a project that compiles it ({@code app} compiling
     *       {@code ../lib/src/main/java}), that project owns it.</li>
     *   <li>Otherwise (a shared folder outside every project, like a {@code core/src/main/java}
     *       that services compile into themselves) a project is made for the folder's tree, named
     *       after it: {@code core}. Its classpath is the union of its users' libraries, the newest
     *       version of each, and its JDK the newest its users compile for.</li>
     * </ul>
     *
     * The other projects drop the folder and depend on the owner instead, and drop their Scala
     * bridge folders for it: the owner exports one.
     */
    static void shareDuplicateRoots(ImportPlan plan, Set<String> names) {
        Map<String, List<Project>> users = new LinkedHashMap<>();
        for (Project p : plan.projects) {
            for (SourceRoot r : p.sources) {
                List<Project> list = users.computeIfAbsent(normalize(r.dir), k -> new ArrayList<>());
                if (!list.contains(p)) {
                    list.add(p);
                }
            }
        }
        Map<Path, Project> sharedByBase = new LinkedHashMap<>();
        for (Map.Entry<String, List<Project>> e : users.entrySet()) {
            if (e.getValue().size() < 2) {
                continue;
            }
            Path dir = Path.of(e.getKey());
            Project owner = null;
            for (Project p : plan.projects) {
                Path projectDir = p.projectDir.toPath().toAbsolutePath().normalize();
                if (dir.startsWith(projectDir)
                    && (owner == null || projectDir.getNameCount() > owner.projectDir.toPath().toAbsolutePath().normalize().getNameCount())) {
                    owner = p;
                }
            }
            if (owner == null || !e.getValue().contains(owner)) {
                Path base = sharedBase(dir);
                Project shared = sharedByBase.computeIfAbsent(base, b -> sharedProject(b, names));
                SourceRoot template = root(e.getValue().get(0), e.getKey());
                SourceRoot root = new SourceRoot();
                root.dir = new File(e.getKey());
                root.sourceSet = template.sourceSet;
                root.generated = template.generated;
                root.test = e.getValue().stream().allMatch(u -> root(u, e.getKey()).test);
                root.link = uniqueLink(links(shared), linkName(shared.projectDir, root.dir));
                shared.sources.add(root);
                for (Project u : e.getValue()) {
                    if (!shared.sharedBy.contains(u)) {
                        shared.sharedBy.add(u);
                    }
                }
                owner = shared;
            }
            for (Project user : e.getValue()) {
                if (user == owner) {
                    continue;
                }
                SourceRoot dropped = root(user, e.getKey());
                user.sources.remove(dropped);
                dependOn(user, owner, dropped.test);
            }
        }
        for (Project shared : sharedByBase.values()) {
            fillSharedProject(plan, shared);
            plan.projects.add(shared);
        }
        // A user's Scala bridge folder for a folder it no longer compiles duplicates the owner's.
        for (Project p : plan.projects) {
            Set<String> own = new LinkedHashSet<>();
            p.sources.forEach(r -> own.add(normalize(r.dir)));
            p.dependencies.removeIf(d -> d.exported && d.file != null && d.sources != null
                && users.containsKey(normalize(d.sources)) && !own.contains(normalize(d.sources)));
        }
    }

    private static Project sharedProject(Path base, Set<String> names) {
        Project shared = new Project();
        shared.projectDir = base.toFile();
        shared.buildName = "shared";
        shared.gradlePath = base.toString();
        shared.name = uniqueName(names, base.getFileName() == null ? "shared" : base.getFileName().toString(), "shared", ":");
        return shared;
    }

    /**
     * The shared project's classpath and compiler settings, from its users: their libraries, the
     * newest version of each, test-only where every user has it test-only; the projects they
     * depend on that do not lead back to a user; the Scala bridge folders for the shared folders;
     * and the compiler settings of the user with the newest language level.
     */
    private static void fillSharedProject(ImportPlan plan, Project shared) {
        Set<String> sharedDirs = new LinkedHashSet<>();
        shared.sources.forEach(r -> sharedDirs.add(normalize(r.dir)));
        Map<String, Project> byName = new HashMap<>();
        plan.projects.forEach(p -> byName.put(p.name, p));
        Map<String, Dependency> byKey = new LinkedHashMap<>();
        Set<String> bridged = new LinkedHashSet<>();
        Project newest = null;
        for (Project user : shared.sharedBy) {
            for (Dependency d : user.dependencies) {
                if (d.exported && d.file != null) {
                    String source = d.sources == null ? null : normalize(d.sources);
                    if (source != null && sharedDirs.contains(source) && bridged.add(source)) {
                        Dependency bridge = copy(d);
                        bridge.test = false;
                        byKey.put("bridge:" + source, bridge);
                    }
                    continue;
                }
                if (d.project != null) {
                    Project target = byName.get(d.project);
                    if (target == null || target == shared || shared.sharedBy.contains(target)
                        || reaches(target, shared.sharedBy, byName, new LinkedHashSet<>())) {
                        continue;
                    }
                }
                String key = d.project != null ? "project:" + d.project
                    : d.coordinates != null ? "module:" + d.coordinates : "file:" + normalize(d.file);
                Dependency existing = byKey.get(key);
                if (existing == null) {
                    byKey.put(key, copy(d));
                } else {
                    existing.test &= d.test;
                    if (d.version != null && compareVersions(d.version, existing.version) > 0) {
                        existing.file = d.file;
                        existing.sources = d.sources;
                        existing.version = d.version;
                    } else if (existing.sources == null && d.file != null && d.file.equals(existing.file)) {
                        existing.sources = d.sources;
                    }
                }
            }
            if (newest == null || level(user.java) > level(newest.java)) {
                newest = user;
            }
        }
        shared.dependencies.addAll(byKey.values());
        if (newest != null) {
            JavaSettings from = newest.java;
            shared.java.javaHome = from.javaHome;
            shared.java.source = from.source;
            shared.java.target = from.target;
            shared.java.release = from.release;
            shared.java.encoding = from.encoding;
            shared.java.parameters = from.parameters;
            shared.java.preview = from.preview;
        }
    }

    /** Whether a project depends, directly or through others, on any of the given projects. */
    private static boolean reaches(Project from, List<Project> targets, Map<String, Project> byName, Set<String> seen) {
        if (!seen.add(from.name)) {
            return false;
        }
        for (Dependency d : from.dependencies) {
            Project next = d.project == null ? null : byName.get(d.project);
            if (next != null && (targets.contains(next) || reaches(next, targets, byName, seen))) {
                return true;
            }
        }
        return false;
    }

    private static void dependOn(Project user, Project owner, boolean test) {
        for (Dependency d : user.dependencies) {
            if (owner.name.equals(d.project)) {
                d.test &= test;
                return;
            }
        }
        Dependency d = new Dependency();
        d.project = owner.name;
        d.test = test;
        user.dependencies.add(d);
    }

    private static SourceRoot root(Project p, String normalizedDir) {
        for (SourceRoot r : p.sources) {
            if (normalize(r.dir).equals(normalizedDir)) {
                return r;
            }
        }
        throw new IllegalStateException(p.name + " has no source root " + normalizedDir);
    }

    private static Set<String> links(Project p) {
        Set<String> links = new LinkedHashSet<>();
        links.add(WorkspaceLayout.OUTPUT_FOLDER);
        p.sources.forEach(r -> links.add(r.link.toLowerCase(Locale.ROOT)));
        return links;
    }

    private static Dependency copy(Dependency d) {
        Dependency c = new Dependency();
        c.file = d.file;
        c.sources = d.sources;
        c.project = d.project;
        c.test = d.test;
        c.exported = d.exported;
        c.coordinates = d.coordinates;
        c.version = d.version;
        return c;
    }

    /**
     * The folder a shared source folder belongs to, which names its project:
     * {@code core/src/main/java} belongs to {@code core}.
     */
    static Path sharedBase(Path dir) {
        Path p = dir;
        for (int up = 0; up < 3 && p != null; up++, p = p.getParent()) {
            if (p.getFileName() != null && p.getFileName().toString().equals("src")) {
                return p.getParent() != null ? p.getParent() : dir;
            }
        }
        return dir;
    }

    /** A Java level as a number: {@code 1.8} is 8, {@code 17} is 17; 0 when unknown. */
    static int level(JavaSettings java) {
        String l = java.release != null ? java.release : java.source;
        if (l == null) {
            return 0;
        }
        String n = l.trim().replace("VERSION_", "").replace('_', '.');
        try {
            return n.startsWith("1.") ? Integer.parseInt(n.substring(2).split("\\.")[0]) : Integer.parseInt(n.split("\\.")[0]);
        } catch (NumberFormatException e) {
            return 0;
        }
    }

    /** Compares versions part by part, numbers as numbers: {@code 2.10 > 2.9}. */
    static int compareVersions(String a, String b) {
        if (b == null) {
            return a == null ? 0 : 1;
        }
        if (a == null) {
            return -1;
        }
        String[] x = a.split("[.\\-+_]");
        String[] y = b.split("[.\\-+_]");
        for (int i = 0; i < Math.max(x.length, y.length); i++) {
            String p = i < x.length ? x[i] : "";
            String q = i < y.length ? y[i] : "";
            int c;
            if (p.matches("\\d+") && q.matches("\\d+")) {
                c = Long.compare(Long.parseLong(p), Long.parseLong(q));
            } else if (p.isEmpty() || q.isEmpty()) {
                c = p.isEmpty() ? (q.matches("\\d+") ? -1 : 1) : (p.matches("\\d+") ? 1 : -1); // 1.0 < 1.0.1, 1.0-rc < 1.0
            } else {
                c = p.compareTo(q);
            }
            if (c != 0) {
                return c;
            }
        }
        return 0;
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

    private static void addResources(Project out, List<JsonObject> sourceSets, Set<String> testSets) {
        Set<String> seen = new LinkedHashSet<>();
        for (JsonObject sourceSet : sourceSets) {
            List<String> dirs = new ArrayList<>(strings(object(object(sourceSet, "sources"), "resources"), "srcDirs"));
            for (JsonElement r : array(sourceSet, "roots")) {
                JsonObject root = r.getAsJsonObject();
                if ("resource".equals(string(root, "kind"))) {
                    dirs.add(string(root, "dir"));
                }
            }
            boolean test = testSets.contains(string(sourceSet, "name"));
            for (String dir : dirs) {
                if (dir != null && seen.add(normalize(new File(dir)))) {
                    (test ? out.testResourceDirs : out.resourceDirs).add(new File(dir));
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

    static String uniqueLink(Set<String> taken, String name) {
        String candidate = name;
        for (int i = 2; taken.contains(candidate.toLowerCase(Locale.ROOT)) || candidate.startsWith("."); i++) {
            candidate = name.replaceFirst("^\\.+", "") + "-" + i;
        }
        taken.add(candidate.toLowerCase(Locale.ROOT));
        return candidate;
    }

    // ---- classpath

    private static void addDependencies(Project out, List<JsonObject> sourceSets, Set<String> testSets, String gradleBuild,
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
                    Project target = byBuildAndPath.get(gradleBuild + "|" + string(entry, "build") + "|" + string(entry, "path"));
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
                Dependency d = addFile(byKey, usedByMain, new File(file), sources == null ? null : new File(sources), test);
                if (d != null && "module".equals(kind) && string(entry, "group") != null && string(entry, "name") != null) {
                    d.coordinates = string(entry, "group") + ":" + string(entry, "name");
                    d.version = string(entry, "version");
                }
            }
        }
        for (Map.Entry<String, Dependency> e : byKey.entrySet()) {
            e.getValue().test = !usedByMain.contains(e.getKey());
            out.dependencies.add(e.getValue());
        }
    }

    private static Dependency addFile(Map<String, Dependency> byKey, Set<String> usedByMain, File file, File sources, boolean test) {
        if (!file.exists()) {
            return null; // JDT would report a missing library as an error on the project
        }
        Dependency d = new Dependency();
        d.file = file;
        d.sources = sources != null && sources.exists() ? sources : null;
        String key = "file:" + normalize(file);
        add(byKey, usedByMain, key, d, test);
        return byKey.get(key);
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
