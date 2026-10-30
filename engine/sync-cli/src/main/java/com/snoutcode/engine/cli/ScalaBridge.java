package com.snoutcode.engine.cli;

import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.gradle.tooling.BuildException;
import org.gradle.tooling.BuildLauncher;
import org.gradle.tooling.ProjectConnection;

import java.io.File;
import java.io.IOException;
import java.io.OutputStream;
import java.io.UncheckedIOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.nio.file.attribute.BasicFileAttributes;
import java.util.ArrayList;
import java.util.Collection;
import java.util.Comparator;
import java.util.HashMap;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TreeSet;
import java.util.concurrent.ConcurrentHashMap;
import java.util.function.Function;
import java.util.function.Supplier;
import java.util.stream.Stream;

/**
 * Puts compiled Scala classes where jdtls can see them. jdtls cannot read Scala source, so Java
 * code that uses Scala types needs their class files on its classpath.
 *
 * <p>For every source set that has Scala files, the bridge runs that source set's Scala compile
 * task, and nothing else, with build directories redirected out of the repository (see
 * {@link Sync#buildDirInitScript}). It then records each source set's Scala classes in the model
 * under {@code scalaBridge.folders}; the jdtls importer adds each folder as a library, with its
 * Scala source root attached. The folders hold only the classes compiled from Scala (see
 * {@link ScalaClassFolder}): jdtls has the Java ones as sources.
 *
 * <p><b>Shared folders, compiled once.</b> Independent builds often compile one Scala folder
 * into each of them (a {@code core/src/main/java} of Java and Scala that services list in their
 * Scala source sets). JDT needs one copy of its classes, owned by the one JDT project the
 * importer makes for the folder, so the bridge compiles such a folder once, as a <i>unit</i>: a
 * task of its own, in one of the builds (the representative: the user with the newest Java),
 * on that build's classpath with the newer or missing libraries of the other users added (their
 * union, as the importer gives the folder's JDT project). Every user then compiles only its own
 * Scala, against the unit's classes. Within the unit and within each build, Java and Scala are
 * still compiled jointly, so each sees the other. A build that takes part compiles into the
 * bridge's own build directories ({@link Sync.Options#bridgeRoot}), apart from the ones Gradle
 * runs for tests and tasks use, which still compile the whole folder as the build says.
 *
 * <p>A unit that fails to compile before it ever worked (a folder that is not self-contained
 * after all) falls back to the build-by-build compile.
 *
 * <p><b>Up to date.</b> Each unit and build has a fingerprint of its inputs (sources, the build's
 * model with its classpath, the units it uses); a start skips the compiles whose fingerprint is
 * the one of their last good compile, so an unchanged project runs no Gradle at all.
 */
public final class ScalaBridge {
    /** Part of every fingerprint: a new bridge recompiles. */
    static final String VERSION = "2";
    /** The prefix of the tasks the bridge registers for its units. */
    static final String SHARED_TASK = "snoutSharedScala";
    static final String SCALA_COMPILER_HEAP = "3g";

    /** The outcome of one compile: a build's Scala tasks, or a unit. */
    public static final class Result {
        public final List<String> tasks = new ArrayList<>();
        public int sourceSets;
        public long millis;
        /** The Gradle run alone, without reading the class folders. */
        public long compileMillis;
        public String error;
        /** Nothing ran: the last compile is still up to date. */
        public boolean upToDate;
    }

    /** One source set with Scala files: one Scala compile task of a build. */
    static final class Compile {
        String buildId;
        JsonObject entry;
        JsonObject sourceSet;
        /** As the root build runs it: {@code :compileScala}, {@code :tools:lib:compileScala}. */
        String task;
        String taskName;
        String sourceSetName;
        /** {@code <build root dir>|<project path>}, how the share script finds the project. */
        String projectKey;
        String module;
        List<Path> srcDirs;
        List<String> includes;
        List<String> excludes;
        Path outputDir;
        /** Binary Scala version ({@code 2.13}, {@code 3}); null when the classpath has no Scala library. */
        String scalaVersion;
        int javaLevel;
        List<JsonObject> classpath;
        /** The active units this compile uses. */
        final List<Unit> units = new ArrayList<>();
        /** Where the bridge compiles to: the build's output folder, or its twin under the bridge root. */
        Path compiledDir;
    }

    /** Source folders several compiles share, compiled once. */
    static final class Unit {
        String key;
        /** For people: the folder's tree, as the importer names its project ({@code core}). */
        String name;
        String scalaVersion;
        List<Path> dirs;
        List<Compile> users;
        Compile rep;
        /** Libraries the other users add to the representative's classpath, and ones they replace. */
        final List<String> add = new ArrayList<>();
        final List<String> drop = new ArrayList<>();
        /** Units compiled earlier whose classes this one may use (all its users use them too). */
        final List<Unit> before = new ArrayList<>();
        Path home;
        boolean active = true;
        List<ScalaClassFolder.Folder> folders = List.of();

        Path output() {
            return home.resolve("classes");
        }

        Path analysis() {
            return home.resolve("analysis").resolve("compile.analysis");
        }

        Path mirrors() {
            return home.resolve("classes-scala");
        }

        String taskName() {
            return SHARED_TASK + "_" + key;
        }

        /** What the readiness reports name it by: the shared source root's name in the model. */
        String module(JsonObject model) {
            for (JsonElement r : array(model, "sharedSourceRoots")) {
                JsonObject root = r.getAsJsonObject();
                String dir = string(root, "dir");
                if (dir != null && dirs.contains(normalize(Path.of(dir))) && string(root, "name") != null) {
                    return string(root, "name");
                }
            }
            return name;
        }
    }

    /** What the bridge compiles for one model, and the state of its last compiles. */
    public static final class Work {
        final JsonObject model;
        final Map<String, ProjectSync.Plan> plans = new LinkedHashMap<>();
        final List<Compile> compiles = new ArrayList<>();
        final List<Unit> units = new ArrayList<>();
        private final Path stateFile;
        private final JsonObject state;

        private Work(JsonObject model, Collection<ProjectSync.Plan> plans, Path stateFile) {
            this.model = model;
            plans.forEach(p -> this.plans.put(p.id(), p));
            this.stateFile = stateFile;
            this.state = readState(stateFile);
        }

        /** The builds with Scala to compile, in the model's order. */
        List<String> builds() {
            Set<String> out = new LinkedHashSet<>();
            compiles.forEach(c -> out.add(c.buildId));
            return new ArrayList<>(out);
        }

        List<Compile> compilesOf(String buildId) {
            return compiles.stream().filter(c -> c.buildId.equals(buildId)).toList();
        }

        /** The active units the build's compiles use. */
        List<Unit> unitsOf(String buildId) {
            Set<Unit> out = new LinkedHashSet<>();
            compilesOf(buildId).forEach(c -> out.addAll(c.units));
            return new ArrayList<>(out);
        }

        /** Whether the build compiles against units, in the bridge's own build directories. */
        boolean bridged(String buildId) {
            return !unitsOf(buildId).isEmpty();
        }

        /** The modules a build's Scala compile makes ready. */
        List<String> modulesOf(String buildId) {
            Set<String> out = new LinkedHashSet<>();
            compilesOf(buildId).forEach(c -> out.add(c.module));
            return new ArrayList<>(out);
        }

        /** Every class folder the model gives jdtls, with its source folder: what jdtls imported. */
        Set<String> folderSet() {
            Set<String> out = new TreeSet<>();
            for (Compile c : compiles) {
                JsonObject bridge = object(c.sourceSet, "scalaBridge");
                for (JsonElement f : array(bridge, "folders")) {
                    out.add(string(f.getAsJsonObject(), "classesDir") + " <- " + string(f.getAsJsonObject(), "sourceDir"));
                }
            }
            return out;
        }

        /** The class folders of a build's compiles, its units' included. */
        List<String> classFolders(String buildId) {
            List<String> out = new ArrayList<>();
            for (Compile c : compilesOf(buildId)) {
                for (JsonElement f : array(object(c.sourceSet, "scalaBridge"), "folders")) {
                    out.add(string(f.getAsJsonObject(), "classesDir"));
                }
            }
            return out;
        }

        // ---- state of the last compiles

        private static JsonObject readState(Path file) {
            if (file != null && Files.isRegularFile(file)) {
                try {
                    JsonObject s = JsonParser.parseString(Files.readString(file)).getAsJsonObject();
                    if (VERSION.equals(string(s, "version"))) {
                        return s;
                    }
                } catch (IOException | RuntimeException e) {
                    // start afresh
                }
            }
            JsonObject s = new JsonObject();
            s.addProperty("version", VERSION);
            s.add("units", new JsonObject());
            s.add("builds", new JsonObject());
            return s;
        }

        synchronized JsonObject recorded(String kind, String key) {
            return object(state.getAsJsonObject(kind), key);
        }

        synchronized void record(String kind, String key, String fingerprint, boolean ok, long millis) {
            JsonObject r = new JsonObject();
            r.addProperty("fingerprint", fingerprint);
            r.addProperty("ok", ok);
            r.addProperty("millis", millis);
            if (fingerprint == null) {
                state.getAsJsonObject(kind).remove(key);
            } else {
                state.getAsJsonObject(kind).add(key, r);
            }
            if (stateFile == null) {
                return;
            }
            try {
                Files.createDirectories(stateFile.getParent());
                Path temp = Files.createTempFile(stateFile.getParent(), "scala-bridge", ".tmp");
                Files.writeString(temp, new GsonBuilder().setPrettyPrinting().create().toJson(state));
                Files.move(temp, stateFile, StandardCopyOption.REPLACE_EXISTING, StandardCopyOption.ATOMIC_MOVE);
            } catch (IOException e) {
                // the next start compiles again
            }
        }

        /** How long the last good compile of a unit or build took, or -1. */
        long lastMillis(String kind, String key) {
            JsonObject r = recorded(kind, key);
            return r == null || !r.has("millis") ? -1 : r.get("millis").getAsLong();
        }
    }

    private ScalaBridge() {
    }

    // ---- planning

    /**
     * Plans the bridge for a model: the Scala compiles of each build that synced, and the units
     * for the Scala folders several of them share. {@code stateFile} keeps the fingerprints of the
     * last compiles across starts; null keeps them in memory.
     */
    public static Work plan(JsonObject model, Collection<ProjectSync.Plan> plans, Path stateFile) {
        Work work = new Work(model, plans, stateFile);
        for (JsonElement g : array(model, "gradleBuilds")) {
            JsonObject entry = g.getAsJsonObject();
            String id = string(entry, "id");
            ProjectSync.Plan plan = work.plans.get(id);
            if (plan == null || plan.needsJdk() != null || !"synced".equals(string(entry, "status"))) {
                continue;
            }
            for (JsonElement b : array(entry, "builds")) {
                JsonObject build = b.getAsJsonObject();
                String kind = string(build, "kind");
                if (!"root".equals(kind) && !"included".equals(kind)) {
                    continue; // buildSrc code is not called from the project's sources
                }
                boolean root = "root".equals(kind);
                String prefix = root ? "" : ":" + string(build, "name");
                String moduleBase = root ? id : id + "/" + string(build, "name");
                String buildRootDir = string(build, "rootDir");
                for (JsonElement p : array(build, "projects")) {
                    JsonObject project = p.getAsJsonObject();
                    String path = string(project, "path");
                    for (JsonElement s : array(project, "sourceSets")) {
                        JsonObject sourceSet = s.getAsJsonObject();
                        JsonObject scala = object(object(sourceSet, "sources"), "scala");
                        String task = string(scala, "compileTask");
                        String outputDir = string(scala, "outputDir");
                        if (task == null || outputDir == null || !hasScalaFiles(scala)) {
                            continue;
                        }
                        Compile c = new Compile();
                        c.buildId = id;
                        c.entry = entry;
                        c.sourceSet = sourceSet;
                        c.task = prefix + task;
                        c.taskName = task.substring(task.lastIndexOf(':') + 1);
                        c.sourceSetName = string(sourceSet, "name");
                        c.projectKey = normalize(Path.of(buildRootDir == null ? string(entry, "rootDir") : buildRootDir)) + "|" + path;
                        c.module = path == null || ":".equals(path) ? moduleBase : moduleBase + path;
                        Set<Path> dirs = new LinkedHashSet<>();
                        strings(scala, "srcDirs").forEach(d -> dirs.add(normalize(Path.of(d))));
                        c.srcDirs = new ArrayList<>(dirs);
                        c.includes = strings(scala, "includes");
                        c.excludes = strings(scala, "excludes");
                        c.outputDir = normalize(Path.of(outputDir));
                        c.classpath = new ArrayList<>();
                        array(sourceSet, "compileClasspath").forEach(e -> c.classpath.add(e.getAsJsonObject()));
                        c.scalaVersion = scalaBinaryVersion(c.classpath);
                        JsonObject compile = object(object(sourceSet, "compile"), "scala");
                        if (compile == null) {
                            compile = object(object(sourceSet, "compile"), "java");
                        }
                        c.javaLevel = level(compile == null ? null
                            : string(compile, "release") != null ? string(compile, "release") : string(compile, "sourceCompatibility"));
                        work.compiles.add(c);
                    }
                }
            }
        }
        planUnits(work);
        for (Compile c : work.compiles) {
            c.compiledDir = compiledDir(work, c);
        }
        return work;
    }

    /** Groups the shared Scala folders into units, picks each one's representative and classpath. */
    private static void planUnits(Work work) {
        // Folder (per Scala version) -> the compiles that list it.
        Map<String, List<Compile>> users = new LinkedHashMap<>();
        for (Compile c : work.compiles) {
            if (!shareable(work, c)) {
                continue;
            }
            for (Path dir : c.srcDirs) {
                if (Files.isDirectory(dir)) {
                    users.computeIfAbsent(c.scalaVersion + "|" + dir, k -> new ArrayList<>()).add(c);
                }
            }
        }
        // Folders with the same users form one unit, compiled together (they may use each other).
        Map<String, List<Path>> dirsBySignature = new LinkedHashMap<>();
        Map<String, List<Compile>> usersBySignature = new LinkedHashMap<>();
        for (Map.Entry<String, List<Compile>> e : users.entrySet()) {
            if (e.getValue().size() < 2) {
                continue;
            }
            String version = e.getKey().substring(0, e.getKey().indexOf('|'));
            Path dir = Path.of(e.getKey().substring(e.getKey().indexOf('|') + 1));
            StringBuilder signature = new StringBuilder(version);
            e.getValue().forEach(c -> signature.append('|').append(work.compiles.indexOf(c)));
            dirsBySignature.computeIfAbsent(signature.toString(), k -> new ArrayList<>()).add(dir);
            usersBySignature.putIfAbsent(signature.toString(), e.getValue());
        }
        for (Map.Entry<String, List<Path>> e : dirsBySignature.entrySet()) {
            Unit u = new Unit();
            u.dirs = e.getValue();
            u.users = usersBySignature.get(e.getKey());
            u.scalaVersion = u.users.get(0).scalaVersion;
            StringBuilder id = new StringBuilder(u.scalaVersion);
            u.dirs.stream().map(Path::toString).sorted().forEach(d -> id.append('\n').append(d));
            u.key = Sync.sha256(id.toString().getBytes(StandardCharsets.UTF_8)).substring(0, 12);
            Path base = sharedBase(u.dirs.get(0));
            u.name = base.getFileName() == null ? "shared" : base.getFileName().toString();
            u.rep = representative(u.users);
            unionClasspath(u);
            ProjectSync.Plan plan = work.plans.get(u.rep.buildId);
            u.home = plan.options().bridgeRoot.toPath().resolve("shared").resolve(u.name.replaceAll("[^A-Za-z0-9._-]", "_") + "-" + u.key);
            work.units.add(u);
        }
        // Wider units first: a narrower one may use their classes, never the other way.
        work.units.sort(Comparator.comparingInt((Unit u) -> -u.users.size()).thenComparing(u -> u.key));
        for (int i = 0; i < work.units.size(); i++) {
            Unit u = work.units.get(i);
            for (int j = 0; j < i; j++) {
                if (work.units.get(j).users.containsAll(u.users)) {
                    u.before.add(work.units.get(j));
                }
            }
        }
        for (Compile c : work.compiles) {
            for (Unit u : work.units) {
                if (u.users.contains(c)) {
                    c.units.add(u);
                }
            }
        }
    }

    /**
     * Whether a compile may take part in a unit: its build runs on Gradle 6.1 or newer (the task
     * properties the share script sets), the bridge has its own build root, the classpath names a
     * Scala library, and its output lies in the build root, so it has a twin under the bridge's.
     */
    private static boolean shareable(Work work, Compile c) {
        ProjectSync.Plan plan = work.plans.get(c.buildId);
        File buildRoot = plan.options().buildRoot;
        return plan.options().bridgeRoot != null && buildRoot != null && c.scalaVersion != null
            && c.outputDir.startsWith(normalize(buildRoot.toPath()))
            && atLeast(string(c.entry, "gradleVersion"), 6, 1);
    }

    /** The user with the newest Java, then the longest classpath, then the first. */
    private static Compile representative(List<Compile> users) {
        Compile best = users.get(0);
        for (Compile c : users) {
            if (c.javaLevel > best.javaLevel || (c.javaLevel == best.javaLevel && c.classpath.size() > best.classpath.size())) {
                best = c;
            }
        }
        return best;
    }

    /**
     * The representative's classpath, made the union of the users': each library another user
     * has in a newer version, or that the representative lacks, is added, and the older one
     * dropped. Other users' project outputs are left out: they are not the shared folder's.
     */
    private static void unionClasspath(Unit u) {
        Map<String, JsonObject> repModules = new HashMap<>();
        Set<String> repFiles = new HashSet<>();
        for (JsonObject e : u.rep.classpath) {
            String file = string(e, "file");
            if (file != null) {
                repFiles.add(normalize(Path.of(file)).toString());
            }
            if ("module".equals(string(e, "kind")) && string(e, "group") != null) {
                repModules.put(string(e, "group") + ":" + string(e, "name"), e);
            }
        }
        Map<String, JsonObject> newer = new LinkedHashMap<>();
        Set<String> files = new LinkedHashSet<>();
        for (Compile user : u.users) {
            if (user == u.rep) {
                continue;
            }
            for (JsonObject e : user.classpath) {
                String file = string(e, "file");
                if (file == null || !Files.exists(Path.of(file))) {
                    continue;
                }
                String kind = string(e, "kind");
                if ("module".equals(kind) && string(e, "group") != null) {
                    String ga = string(e, "group") + ":" + string(e, "name");
                    JsonObject mine = repModules.get(ga);
                    if (mine != null && compareVersions(string(e, "version"), string(mine, "version")) <= 0) {
                        continue;
                    }
                    JsonObject current = newer.get(ga);
                    if (current == null || compareVersions(string(e, "version"), string(current, "version")) > 0) {
                        newer.put(ga, e);
                    }
                } else if ("file".equals(kind) && !repFiles.contains(normalize(Path.of(file)).toString())) {
                    files.add(normalize(Path.of(file)).toString());
                }
            }
        }
        for (Map.Entry<String, JsonObject> e : newer.entrySet()) {
            JsonObject mine = repModules.get(e.getKey());
            if (mine != null) {
                u.drop.add(normalize(Path.of(string(mine, "file"))).toString());
            }
            u.add.add(normalize(Path.of(string(e.getValue(), "file"))).toString());
        }
        u.add.addAll(files);
    }

    /** Where a compile's classes land: its output folder, or for a build with units, the twin under the bridge root. */
    private static Path compiledDir(Work work, Compile c) {
        if (!work.bridged(c.buildId)) {
            return c.outputDir;
        }
        Sync.Options o = work.plans.get(c.buildId).options();
        Path buildRoot = normalize(o.buildRoot.toPath());
        return normalize(o.bridgeRoot.toPath()).resolve(buildRoot.relativize(c.outputDir));
    }

    // ---- the model's annotations

    /**
     * Records in the model the class folders each Scala source set gives jdtls, before or without
     * compiling: the folders are stable (see {@link ScalaClassFolder}) and created empty, so jdtls
     * can import them at once, and a later compile only fills them.
     */
    public static void annotate(Work work) throws IOException {
        for (Unit u : work.units) {
            if (u.active) {
                u.folders = ScalaClassFolder.planned(u.mirrors(), u.dirs);
            }
        }
        for (Compile c : work.compiles) {
            c.compiledDir = compiledDir(work, c);
            annotate(c, ScalaClassFolder.planned(mirrorRoot(c.compiledDir), ownRoots(c)));
        }
    }

    private static void annotate(Compile c, List<ScalaClassFolder.Folder> own) {
        JsonObject bridge = new JsonObject();
        bridge.addProperty("compiledDir", c.compiledDir.toString());
        JsonArray folders = new JsonArray();
        List<ScalaClassFolder.Folder> all = new ArrayList<>(own);
        JsonArray shared = new JsonArray();
        for (Unit u : c.units) {
            all.addAll(u.folders);
            shared.add(u.key);
        }
        for (ScalaClassFolder.Folder folder : all) {
            JsonObject f = new JsonObject();
            f.addProperty("classesDir", folder.classesDir().toString());
            f.addProperty("sourceDir", folder.sourceDir() == null ? null : folder.sourceDir().toString());
            f.addProperty("classes", folder.classes());
            folders.add(f);
        }
        bridge.add("folders", folders);
        if (!shared.isEmpty()) {
            bridge.add("units", shared);
        }
        c.sourceSet.add("scalaBridge", bridge);
    }

    /** A compile's Scala folders that no active unit compiles for it. */
    private static List<Path> ownRoots(Compile c) {
        List<Path> out = new ArrayList<>(c.srcDirs);
        c.units.forEach(u -> out.removeAll(u.dirs));
        return out;
    }

    private static Path mirrorRoot(Path compiled) {
        return compiled.resolveSibling(compiled.getFileName() + "-scala");
    }

    // ---- fingerprints

    /** What a unit's compile depends on: its folders' files, the representative's build, its classpath changes. */
    static String fingerprint(Unit u) {
        StringBuilder s = new StringBuilder("unit " + VERSION + "\n" + u.scalaVersion + "\n");
        u.dirs.forEach(d -> s.append(d).append('\n').append(filesDigest(List.of(d))).append('\n'));
        s.append(u.rep.buildId).append(' ').append(u.rep.task).append('\n').append(canonicalHash(u.rep.entry)).append('\n');
        s.append(u.add).append('\n').append(u.drop).append('\n');
        u.before.forEach(b -> s.append(b.key).append(b.active).append('\n'));
        return Sync.sha256(s.toString().getBytes(StandardCharsets.UTF_8));
    }

    /** What a build's compile depends on: its model, the sources of its Scala projects, the units it uses. */
    static String fingerprint(Work work, String buildId) {
        List<Compile> compiles = work.compilesOf(buildId);
        StringBuilder s = new StringBuilder("build " + VERSION + "\n");
        s.append(canonicalHash(compiles.get(0).entry)).append('\n');
        Set<Path> dirs = new TreeSet<>();
        Set<Unit> units = new LinkedHashSet<>();
        for (Compile c : compiles) {
            s.append(c.task).append(' ').append(c.compiledDir).append('\n');
            units.addAll(c.units);
            dirs.addAll(ownRoots(c));
        }
        // The other source folders of the same projects: Scala compiles against their classes.
        for (JsonElement b : array(compiles.get(0).entry, "builds")) {
            for (JsonElement p : array(b.getAsJsonObject(), "projects")) {
                JsonObject project = p.getAsJsonObject();
                boolean scala = false;
                for (JsonElement ss : array(project, "sourceSets")) {
                    scala |= compiles.stream().anyMatch(c -> c.sourceSet == ss.getAsJsonObject());
                }
                if (!scala) {
                    continue;
                }
                for (JsonElement ss : array(project, "sourceSets")) {
                    JsonObject sources = object(ss.getAsJsonObject(), "sources");
                    for (String language : List.of("java", "scala", "groovy", "kotlin")) {
                        strings(object(sources, language), "srcDirs").forEach(d -> dirs.add(normalize(Path.of(d))));
                    }
                }
            }
        }
        for (Unit u : units) {
            dirs.removeAll(u.dirs);
            s.append("unit ").append(u.key).append(' ').append(u.active).append('\n');
        }
        s.append(filesDigest(new ArrayList<>(dirs)));
        return Sync.sha256(s.toString().getBytes(StandardCharsets.UTF_8));
    }

    /** Path, size and modification time of every Java and Scala file under the folders. */
    static String filesDigest(List<Path> dirs) {
        StringBuilder s = new StringBuilder();
        for (Path dir : dirs) {
            if (!Files.isDirectory(dir)) {
                s.append(dir).append(" missing\n");
                continue;
            }
            List<String> lines = new ArrayList<>();
            try {
                Files.walkFileTree(dir, new java.nio.file.SimpleFileVisitor<>() {
                    @Override
                    public java.nio.file.FileVisitResult visitFile(Path file, BasicFileAttributes attrs) {
                        String name = file.getFileName().toString();
                        if (name.endsWith(".scala") || name.endsWith(".java")) {
                            lines.add(dir.relativize(file) + " " + attrs.size() + " " + attrs.lastModifiedTime().toMillis());
                        }
                        return java.nio.file.FileVisitResult.CONTINUE;
                    }
                });
            } catch (IOException e) {
                throw new UncheckedIOException(e);
            }
            lines.sort(null);
            s.append(dir).append('\n');
            lines.forEach(l -> s.append(l).append('\n'));
        }
        return Sync.sha256(s.toString().getBytes(StandardCharsets.UTF_8));
    }

    /**
     * A hash of a build's model, without what changes from one sync or compile to the next, the
     * same for the model in memory and as read back from its file.
     */
    static String canonicalHash(JsonElement e) {
        StringBuilder s = new StringBuilder();
        canonical(e, s);
        return Sync.sha256(s.toString().getBytes(StandardCharsets.UTF_8));
    }

    private static final Set<String> VOLATILE = Set.of("syncMillis", "buildMillis", "scalaBridge");

    private static void canonical(JsonElement e, StringBuilder s) {
        if (e.isJsonObject()) {
            s.append('{');
            for (Map.Entry<String, JsonElement> m : e.getAsJsonObject().entrySet()) {
                // A null is left out, as the model file leaves it out.
                if (!VOLATILE.contains(m.getKey()) && !m.getValue().isJsonNull()) {
                    s.append(m.getKey()).append(':');
                    canonical(m.getValue(), s);
                    s.append(',');
                }
            }
            s.append('}');
        } else if (e.isJsonArray()) {
            s.append('[');
            for (JsonElement x : e.getAsJsonArray()) {
                canonical(x, s);
                s.append(',');
            }
            s.append(']');
        } else {
            s.append(e);
        }
    }

    // ---- compiling

    /** Hears how a {@link #run} goes; called from the compiling threads. */
    public interface Listener {
        default void unitStarted(Unit unit) {
        }

        default void unitFinished(Unit unit, Result result) {
        }

        default void buildStarted(String buildId) {
        }

        default void buildFinished(String buildId, Result result) {
        }
    }

    /** What a {@link #run} compiles: these builds (null: every one), the ones forced even when up to date. */
    public record Selection(Set<String> builds, Set<String> forcedBuilds, Set<String> forcedUnits) {
        static Selection all() {
            return new Selection(null, Set.of(), Set.of());
        }
    }

    /**
     * Compiles what the selection needs, several builds at once: first the units (each before the
     * builds that use it), then the builds, those {@code preferred} names (the builds of the files
     * open in the editor) first. A unit or build whose fingerprint is that of its last good
     * compile is skipped unless forced. Returns each build's result.
     */
    public static Map<String, Result> run(Work work, Selection selection, int parallel, Supplier<Collection<String>> preferred,
                                          Listener listener, Function<String, OutputStream> output) throws IOException {
        List<String> builds = new ArrayList<>();
        for (String b : work.builds()) {
            if (selection.builds() == null || selection.builds().contains(b) || selection.forcedBuilds().contains(b)) {
                builds.add(b);
            }
        }
        List<Unit> units = new ArrayList<>();
        for (Unit u : work.units) {
            boolean used = selection.forcedUnits().contains(u.key)
                || u.users.stream().anyMatch(c -> builds.contains(c.buildId));
            if (used && u.active) {
                units.add(u);
            }
        }
        Scheduler scheduler = new Scheduler(work, units, builds, preferred);
        Map<String, Result> results = new ConcurrentHashMap<>();
        int threads = Math.max(1, Math.min(parallel, units.size() + builds.size()));
        List<Thread> workers = new ArrayList<>();
        List<Throwable> failures = new ArrayList<>();
        for (int i = 0; i < threads; i++) {
            Thread t = new Thread(() -> {
                try {
                    Object next;
                    while ((next = scheduler.next()) != null) {
                        try {
                            if (next instanceof Unit u) {
                                listener.unitStarted(u);
                                Result r = runUnit(work, u, selection.forcedUnits().contains(u.key), output);
                                listener.unitFinished(u, r);
                            } else {
                                String b = (String) next;
                                listener.buildStarted(b);
                                Result r = runBuild(work, b, selection.forcedBuilds().contains(b), output);
                                results.put(b, r);
                                listener.buildFinished(b, r);
                            }
                        } finally {
                            scheduler.done(next);
                        }
                    }
                } catch (InterruptedException e) {
                    Thread.currentThread().interrupt();
                } catch (RuntimeException | Error e) {
                    synchronized (failures) {
                        failures.add(e);
                    }
                    scheduler.abort();
                }
            }, "snout-scala-" + i);
            t.setDaemon(true);
            t.start();
            workers.add(t);
        }
        try {
            for (Thread t : workers) {
                t.join();
            }
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            throw new IOException("interrupted", e);
        }
        if (!failures.isEmpty()) {
            throw new IOException(failures.get(0));
        }
        return results;
    }

    /**
     * How long the first build handed out after a unit compiled goes alone, so that it, the
     * longest or the open files', gets the daemon the unit compiled on: warm, its Scala compiler
     * loaded. A build that starts a daemon of its own took 10 s longer to compile.
     */
    static final long HEAD_START_MILLIS = 1500;

    /** Hands out units and builds as their units are done. */
    private static final class Scheduler {
        private final Work work;
        private final List<Unit> units;
        private final List<String> builds;
        private final Set<Object> running = new HashSet<>();
        private final Supplier<Collection<String>> preferred;
        private final Map<String, Long> costs = new HashMap<>();
        private boolean aborted;
        /** A unit finished since a build was last handed out. */
        private boolean unitDone;
        /** Until when no other build is handed out (nanoTime). */
        private long headStartUntil;

        Scheduler(Work work, List<Unit> units, List<String> builds, Supplier<Collection<String>> preferred) {
            this.work = work;
            this.units = new ArrayList<>(units);
            this.builds = new ArrayList<>(builds);
            this.preferred = preferred == null ? List::of : preferred;
        }

        synchronized Object next() throws InterruptedException {
            while (!aborted) {
                for (Unit u : units) {
                    if (u.before.stream().noneMatch(b -> units.contains(b) || running.contains(b))) {
                        units.remove(u);
                        running.add(u);
                        return u;
                    }
                }
                List<String> ready = builds.stream()
                    .filter(b -> work.unitsOf(b).stream().noneMatch(u -> units.contains(u) || running.contains(u)))
                    .toList();
                long now = System.nanoTime();
                if (!ready.isEmpty() && now < headStartUntil) {
                    // The build handed out just now goes first to the daemon a unit warmed.
                    wait(Math.max(1, (headStartUntil - now) / 1_000_000));
                    continue;
                }
                if (!ready.isEmpty()) {
                    Collection<String> first = preferred.get();
                    // The open files' builds first, then the longest compiles, so the last to
                    // finish is a short one.
                    String pick = ready.stream().filter(first::contains).findFirst()
                        .orElseGet(() -> ready.stream().max(Comparator.comparingLong(this::cost)).orElseThrow());
                    builds.remove(pick);
                    running.add(pick);
                    if (unitDone) {
                        unitDone = false;
                        headStartUntil = System.nanoTime() + HEAD_START_MILLIS * 1_000_000;
                    }
                    return pick;
                }
                if (units.isEmpty() && builds.isEmpty()) {
                    return null;
                }
                wait();
            }
            return null;
        }

        synchronized void done(Object item) {
            running.remove(item);
            if (item instanceof Unit) {
                unitDone = true;
            }
            notifyAll();
        }

        /**
         * What a build's compile is likely to take: its last good one, else the Scala and Java
         * files of its own source folders (the units' folders count for none: they are compiled).
         */
        private long cost(String build) {
            long last = work.lastMillis("builds", build);
            if (last > 0) {
                return last;
            }
            return costs.computeIfAbsent(build, b -> {
                Set<Path> shared = new HashSet<>();
                work.unitsOf(b).forEach(u -> shared.addAll(u.dirs));
                long files = 0;
                for (Compile c : work.compilesOf(b)) {
                    for (Path dir : c.srcDirs) {
                        if (shared.contains(dir) || !Files.isDirectory(dir)) {
                            continue;
                        }
                        try (Stream<Path> walk = Files.walk(dir)) {
                            files += walk.filter(f -> f.toString().endsWith(".scala") || f.toString().endsWith(".java")).count();
                        } catch (IOException | UncheckedIOException e) {
                            // an unreadable folder counts for nothing
                        }
                    }
                }
                return files;
            });
        }

        synchronized void abort() {
            aborted = true;
            notifyAll();
        }
    }

    private static Result runUnit(Work work, Unit u, boolean forced, Function<String, OutputStream> output) {
        long start = System.nanoTime();
        String fingerprint = fingerprint(u);
        JsonObject last = work.recorded("units", u.key);
        boolean everWorked = last != null && last.get("ok").getAsBoolean() || hasClasses(u.output());
        Result result = new Result();
        result.tasks.add(u.rep.buildId + ":" + u.taskName());
        if (!forced && last != null && fingerprint.equals(string(last, "fingerprint"))) {
            if (last.get("ok").getAsBoolean() && Files.isDirectory(u.output())) {
                result.upToDate = true;
                result.millis = (System.nanoTime() - start) / 1_000_000;
                return result;
            }
            if (!last.get("ok").getAsBoolean() && !everWorked) {
                deactivate(work, u);
                result.error = "the shared folders did not compile on their own; compiling them in each build";
                result.upToDate = true;
                return result;
            }
        }
        ProjectSync.Plan plan = work.plans.get(u.rep.buildId);
        Path spec;
        try {
            spec = writeSpec(work, u.rep.buildId);
        } catch (IOException e) {
            result.error = SnoutEngine.describe(e);
            return result;
        }
        String projectPath = u.rep.task.substring(0, u.rep.task.length() - u.rep.taskName.length());
        gradle(work, plan, List.of(projectPath + u.taskName()), spec, output.apply(u.rep.buildId), result);
        try {
            if (result.error == null) {
                u.folders = ScalaClassFolder.mirror(u.output(), u.mirrors(), u.dirs);
                work.record("units", u.key, fingerprint, true, result.compileMillis);
            } else if (!everWorked) {
                work.record("units", u.key, fingerprint, false, result.compileMillis);
                deactivate(work, u);
            } else {
                // The last good classes stay: the users compile against them.
                u.folders = ScalaClassFolder.mirror(u.output(), u.mirrors(), u.dirs);
                work.record("units", u.key, null, false, 0);
            }
            for (Compile c : u.users) {
                JsonObject bridge = object(c.sourceSet, "scalaBridge");
                if (bridge != null) {
                    annotate(c, ownFolders(c));
                }
            }
        } catch (IOException e) {
            result.error = result.error != null ? result.error : SnoutEngine.describe(e);
        }
        result.sourceSets = u.folders.size();
        result.millis = (System.nanoTime() - start) / 1_000_000;
        return result;
    }

    /** A unit that never compiled on its own: its users compile its folders themselves again. */
    private static void deactivate(Work work, Unit u) {
        u.active = false;
        for (Compile c : u.users) {
            c.units.remove(u);
        }
        for (Compile c : work.compiles) {
            c.compiledDir = compiledDir(work, c);
            try {
                annotate(c, ScalaClassFolder.planned(mirrorRoot(c.compiledDir), ownRoots(c)));
            } catch (IOException e) {
                throw new UncheckedIOException(e);
            }
        }
    }

    /** The folders the last annotation gave a compile, without its units'. */
    private static List<ScalaClassFolder.Folder> ownFolders(Compile c) throws IOException {
        return ScalaClassFolder.planned(mirrorRoot(c.compiledDir), ownRoots(c));
    }

    private static Result runBuild(Work work, String buildId, boolean forced, Function<String, OutputStream> output) {
        long start = System.nanoTime();
        Result result = new Result();
        List<Compile> compiles = work.compilesOf(buildId);
        compiles.forEach(c -> {
            if (!result.tasks.contains(c.task)) {
                result.tasks.add(c.task);
            }
        });
        String fingerprint = fingerprint(work, buildId);
        JsonObject last = work.recorded("builds", buildId);
        boolean present = compiles.stream().allMatch(c -> Files.isDirectory(c.compiledDir));
        if (!forced && present && last != null && last.get("ok").getAsBoolean() && fingerprint.equals(string(last, "fingerprint"))) {
            result.upToDate = true;
        } else {
            Path spec = null;
            try {
                if (work.bridged(buildId)) {
                    spec = writeSpec(work, buildId);
                }
            } catch (IOException e) {
                result.error = SnoutEngine.describe(e);
                return result;
            }
            gradle(work, work.plans.get(buildId), result.tasks, spec, output.apply(buildId), result);
            work.record("builds", buildId, result.error == null ? fingerprint : null, result.error == null, result.compileMillis);
        }
        try {
            for (Compile c : compiles) {
                annotate(c, ScalaClassFolder.mirror(c.compiledDir, mirrorRoot(c.compiledDir), ownRoots(c)));
                result.sourceSets++;
            }
        } catch (IOException e) {
            result.error = result.error != null ? result.error : SnoutEngine.describe(e);
        }
        result.millis = (System.nanoTime() - start) / 1_000_000;
        return result;
    }

    /** Runs Gradle's tasks for a build, the share script given a spec; fills in the result's error and time. */
    private static void gradle(Work work, ProjectSync.Plan plan, List<String> tasks, Path spec, OutputStream output, Result result) {
        long start = System.nanoTime();
        Sync.Options options = plan.options();
        try {
            List<Path> initScripts = new ArrayList<>();
            File root = spec != null ? options.bridgeRoot : options.buildRoot;
            if (root != null) {
                initScripts.add(Sync.buildDirInitScript(root));
            }
            initScripts.add(compilerMemoryInitScript());
            if (spec != null) {
                initScripts.add(shareInitScript());
            }
            try (ProjectConnection connection = Sync.connector(plan.rootDir(), options).connect()) {
                List<String> args = Sync.arguments(initScripts, options.projectCacheDir, options);
                args.add("--continue");
                if (spec != null) {
                    args.add("-D" + SPEC_PROPERTY + "=" + spec.toAbsolutePath());
                }
                BuildLauncher launcher = connection.newBuild().forTasks(tasks.toArray(new String[0])).withArguments(args);
                if (options.javaHome != null) {
                    launcher.setJavaHome(options.javaHome);
                }
                OutputStream out = output != null ? output : OutputStream.nullOutputStream();
                launcher.setStandardOutput(out);
                launcher.setStandardError(out);
                launcher.setColorOutput(false);
                launcher.run();
            }
        } catch (BuildException e) {
            result.error = SnoutEngine.describe(e);
        } catch (IOException | RuntimeException e) {
            result.error = SnoutEngine.describe(e);
        }
        result.compileMillis = (System.nanoTime() - start) / 1_000_000;
    }

    // ---- the share script and its spec

    static final String SPEC_PROPERTY = "snout.scalaShare";

    /**
     * The spec of one build for the share script: which source sets leave out which shared
     * folders, the units' classes each project compiles against, and the unit tasks to register
     * in it. A build gets the same spec for each of its runs, so its tasks stay up to date.
     */
    static Path writeSpec(Work work, String buildId) throws IOException {
        JsonObject spec = new JsonObject();
        Set<String> classes = new LinkedHashSet<>();
        JsonArray users = new JsonArray();
        JsonArray units = new JsonArray();
        for (Compile c : work.compilesOf(buildId)) {
            if (c.units.isEmpty()) {
                continue;
            }
            JsonObject user = new JsonObject();
            user.addProperty("project", c.projectKey);
            user.addProperty("sourceSet", c.sourceSetName);
            JsonArray exclude = new JsonArray();
            JsonArray userClasses = new JsonArray();
            for (Unit u : c.units) {
                u.dirs.forEach(d -> exclude.add(d.toString()));
                userClasses.add(u.output().toString());
                classes.add(u.output().toString());
            }
            user.add("exclude", exclude);
            user.add("classes", userClasses);
            users.add(user);
        }
        for (Unit u : work.units) {
            if (!u.active || !u.rep.buildId.equals(buildId)) {
                continue;
            }
            JsonObject unit = new JsonObject();
            unit.addProperty("project", u.rep.projectKey);
            unit.addProperty("task", u.taskName());
            unit.addProperty("from", u.rep.taskName);
            JsonArray dirs = new JsonArray();
            u.dirs.forEach(d -> dirs.add(d.toString()));
            unit.add("dirs", dirs);
            unit.add("includes", strings(u.rep.includes));
            unit.add("excludes", strings(u.rep.excludes));
            unit.addProperty("output", u.output().toString());
            unit.addProperty("analysis", u.analysis().toString());
            // Off the classpath: replaced libraries, and every unit's classes but those it may use.
            List<String> drop = new ArrayList<>(u.drop);
            for (Unit other : work.units) {
                if (!u.before.contains(other)) {
                    drop.add(other.output().toString());
                }
            }
            unit.add("drop", strings(drop));
            unit.add("add", strings(u.add));
            List<String> before = new ArrayList<>();
            u.before.stream().filter(b -> b.active).forEach(b -> before.add(b.output().toString()));
            unit.add("classes", strings(before));
            units.add(unit);
        }
        spec.add("classes", strings(new ArrayList<>(classes)));
        spec.add("users", users);
        spec.add("units", units);
        Path dir = work.plans.get(buildId).options().bridgeRoot.toPath().resolve("specs");
        Files.createDirectories(dir);
        Path file = dir.resolve(ProjectSync.key(work.plans.get(buildId).rootDir()) + ".json");
        String text = new GsonBuilder().setPrettyPrinting().create().toJson(spec);
        if (!Files.isRegularFile(file) || !Files.readString(file).equals(text)) {
            Files.writeString(file, text);
        }
        return file;
    }

    /**
     * The share script: reads the spec named by {@value #SPEC_PROPERTY}; leaves the shared
     * folders out of their users' Scala compile; puts the units' classes on the classpath of every
     * compile task of the build but the Java (and Groovy, Kotlin) compile of the source sets that
     * leave them out, which run before the Scala compile and never saw those classes; and
     * registers each unit's task, made from its representative's Scala compile.
     */
    static Path shareInitScript() throws IOException {
        String text = """
            // Generated by snout-engine: compiles Scala folders that several builds share once.
            def snoutSpecFile = System.getProperty('%s')
            if (snoutSpecFile == null || snoutSpecFile.isEmpty()) {
                return
            }
            def snoutSpec = new groovy.json.JsonSlurper().parse(new File(snoutSpecFile))
            def snoutNorm = { String s -> new File(s).absoluteFile.toPath().normalize().toString() }
            gradle.allprojects { p ->
                def key = snoutNorm(p.rootDir.path) + '|' + p.path
                def users = snoutSpec.users.findAll { it.project == key }
                def units = snoutSpec.units.findAll { it.project == key }
                def extra = users.isEmpty() ? snoutSpec.classes : users.collectMany { it.classes }.unique()
                p.plugins.withId('java') {
                    def skip = [] as Set
                    users.each { u ->
                        def ss = p.sourceSets.findByName(u.sourceSet)
                        if (ss == null) {
                            return
                        }
                        ['java', 'groovy', 'kotlin'].each { skip << ss.getCompileTaskName(it) }
                        def dirs = u.exclude.collect { snoutNorm(it) + File.separator }
                        def scalaTask = ss.getCompileTaskName('scala')
                        p.tasks.matching { it.name == scalaTask }.configureEach { t ->
                            t.exclude { e -> def f = e.file.absolutePath; dirs.any { f.startsWith(it) } }
                        }
                    }
                    if (!extra.isEmpty()) {
                        // After the plugins and the build script have set each task's classpath.
                        p.afterEvaluate {
                            p.tasks.withType(AbstractCompile).configureEach { t ->
                                if (!t.name.startsWith('%s') && !skip.contains(t.name)) {
                                    t.classpath = t.classpath + p.files(extra)
                                }
                            }
                        }
                    }
                }
                p.plugins.withId('scala') {
                    units.each { unit ->
                        p.tasks.register(unit.task, ScalaCompile) { t ->
                            def from = p.tasks.getByName(unit.from)
                            def drop = unit.drop.collect { snoutNorm(it) } as Set
                            t.classpath = from.classpath.filter { !drop.contains(snoutNorm(it.path)) } + p.files(unit.add) + p.files(unit.classes)
                            unit.dirs.each { t.source(it) }
                            if (unit.includes.isEmpty()) {
                                t.include('**/*.java', '**/*.scala')
                            } else {
                                t.include(unit.includes)
                            }
                            t.exclude(unit.excludes)
                            t.destinationDirectory.set(new File(unit.output))
                            def incremental = t.scalaCompileOptions.incrementalOptions
                            incremental.analysisFile.set(new File(unit.analysis))
                            try { incremental.classfileBackupDir.set(new File(unit.analysis + '.backup')) } catch (Throwable ignored) { }
                            try { t.analysisMappingFile.set(new File(unit.analysis + '.mapping')) } catch (Throwable ignored) { }
                            t.sourceCompatibility = from.sourceCompatibility
                            t.targetCompatibility = from.targetCompatibility
                            try { t.javaLauncher.set(from.javaLauncher) } catch (Throwable ignored) { }
                            t.options.encoding = from.options.encoding
                            t.options.compilerArgs = from.options.compilerArgs
                            t.scalaCompileOptions.additionalParameters = from.scalaCompileOptions.additionalParameters
                            t.scalaCompileOptions.forkOptions.memoryMaximumSize = from.scalaCompileOptions.forkOptions.memoryMaximumSize
                            t.scalaCompileOptions.forkOptions.jvmArgs = from.scalaCompileOptions.forkOptions.jvmArgs
                        }
                    }
                }
            }
            """.formatted(SPEC_PROPERTY, SHARED_TASK);
        return Sync.writeScript("snout-scala-share", text);
    }

    /**
     * Gives the Scala compiler daemon a heap of {@value #SCALA_COMPILER_HEAP} when the build
     * sets none: Gradle starts it with the JVM's small default otherwise, which a large source
     * set (thousands of files, compiled jointly with Java) runs out of.
     */
    static Path compilerMemoryInitScript() throws IOException {
        String text = "// Generated by snout-engine: room for the Scala compiler when the build gives it none.\n"
            + "gradle.allprojects { p ->\n"
            + "    p.tasks.withType(ScalaCompile).configureEach { t ->\n"
            + "        def fork = t.scalaCompileOptions.forkOptions\n"
            + "        if (fork.memoryMaximumSize == null) {\n"
            + "            fork.memoryMaximumSize = '" + SCALA_COMPILER_HEAP + "'\n"
            + "        }\n"
            + "    }\n"
            + "}\n";
        return Sync.writeScript("snout-scala-memory", text);
    }

    // ---- which builds and units hold files

    /** The builds whose own Scala folders hold any of the files. */
    static Set<String> buildsHolding(Work work, List<Path> files) {
        Set<String> out = new LinkedHashSet<>();
        for (Compile c : work.compiles) {
            for (Path dir : ownRoots(c)) {
                if (files.stream().anyMatch(f -> normalize(f).startsWith(dir))) {
                    out.add(c.buildId);
                }
            }
        }
        return out;
    }

    /** The active units whose folders hold any of the files. */
    static Set<Unit> unitsHolding(Work work, List<Path> files) {
        Set<Unit> out = new LinkedHashSet<>();
        for (Unit u : work.units) {
            if (u.active && u.dirs.stream().anyMatch(d -> files.stream().anyMatch(f -> normalize(f).startsWith(d)))) {
                out.add(u);
            }
        }
        return out;
    }

    /** Whether any of these files lies in a Scala source folder of the build's model. */
    static boolean holdsAny(JsonObject model, List<Path> files) {
        for (JsonObject scala : scalaSources(model)) {
            for (JsonElement d : scala.getAsJsonArray("srcDirs")) {
                Path dir = Path.of(d.getAsString()).toAbsolutePath().normalize();
                for (Path f : files) {
                    if (f.toAbsolutePath().normalize().startsWith(dir)) {
                        return true;
                    }
                }
            }
        }
        return false;
    }

    /** The class folders the last compile recorded for jdtls, across the build. */
    static List<String> classFolders(JsonObject model) {
        List<String> out = new ArrayList<>();
        for (JsonElement b : model.getAsJsonArray("builds")) {
            for (JsonElement p : b.getAsJsonObject().getAsJsonArray("projects")) {
                JsonObject project = p.getAsJsonObject();
                if (!project.has("sourceSets")) {
                    continue;
                }
                for (JsonElement s : project.getAsJsonArray("sourceSets")) {
                    JsonObject bridge = object(s.getAsJsonObject(), "scalaBridge");
                    if (bridge == null || !bridge.has("folders")) {
                        continue;
                    }
                    for (JsonElement f : bridge.getAsJsonArray("folders")) {
                        out.add(f.getAsJsonObject().get("classesDir").getAsString());
                    }
                }
            }
        }
        return out;
    }

    private static List<JsonObject> scalaSources(JsonObject model) {
        List<JsonObject> out = new ArrayList<>();
        for (JsonElement b : model.getAsJsonArray("builds")) {
            for (JsonElement p : b.getAsJsonObject().getAsJsonArray("projects")) {
                JsonObject project = p.getAsJsonObject();
                if (!project.has("sourceSets")) {
                    continue;
                }
                for (JsonElement s : project.getAsJsonArray("sourceSets")) {
                    JsonObject scala = object(object(s.getAsJsonObject(), "sources"), "scala");
                    if (scala != null && scala.has("srcDirs")) {
                        out.add(scala);
                    }
                }
            }
        }
        return out;
    }

    // ---- helpers

    private static boolean hasScalaFiles(JsonObject scala) {
        if (scala == null || !scala.has("srcDirs")) {
            return false;
        }
        for (JsonElement d : scala.getAsJsonArray("srcDirs")) {
            Path dir = Path.of(d.getAsString());
            if (!Files.isDirectory(dir)) {
                continue;
            }
            try (Stream<Path> files = Files.walk(dir)) {
                if (files.anyMatch(f -> f.getFileName().toString().endsWith(".scala"))) {
                    return true;
                }
            } catch (IOException e) {
                throw new UncheckedIOException(e);
            }
        }
        return false;
    }

    private static boolean hasClasses(Path dir) {
        if (!Files.isDirectory(dir)) {
            return false;
        }
        try (Stream<Path> walk = Files.walk(dir)) {
            return walk.anyMatch(p -> p.toString().endsWith(".class"));
        } catch (IOException e) {
            return false;
        }
    }

    /** The Scala library's binary version on a classpath: {@code 2.13}, or {@code 3}. */
    static String scalaBinaryVersion(List<JsonObject> classpath) {
        for (JsonObject e : classpath) {
            String name = string(e, "name");
            String version = string(e, "version");
            if (!"org.scala-lang".equals(string(e, "group")) || name == null || version == null) {
                continue;
            }
            if (name.equals("scala3-library_3")) {
                return "3";
            }
            if (name.equals("scala-library")) {
                String[] parts = version.split("\\.");
                return parts.length >= 2 ? parts[0] + "." + parts[1] : version;
            }
        }
        return null;
    }

    /** A Java level as a number: {@code 1.8} is 8, {@code 17} is 17; 0 when unknown. */
    static int level(String l) {
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

    private static boolean atLeast(String version, int major, int minor) {
        if (version == null) {
            return false;
        }
        String[] parts = version.split("[.\\-]");
        try {
            int ma = Integer.parseInt(parts[0]);
            int mi = parts.length > 1 ? Integer.parseInt(parts[1]) : 0;
            return ma > major || (ma == major && mi >= minor);
        } catch (NumberFormatException e) {
            return false;
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
                c = p.isEmpty() ? (q.matches("\\d+") ? -1 : 1) : (p.matches("\\d+") ? 1 : -1);
            } else {
                c = p.compareTo(q);
            }
            if (c != 0) {
                return c;
            }
        }
        return 0;
    }

    /** The folder a shared source folder belongs to, as the importer names its project: {@code core/src/main/java} is {@code core}. */
    static Path sharedBase(Path dir) {
        Path p = dir;
        for (int up = 0; up < 3 && p != null; up++, p = p.getParent()) {
            if (p.getFileName() != null && p.getFileName().toString().equals("src")) {
                return p.getParent() != null ? p.getParent() : dir;
            }
        }
        return dir;
    }

    static Path normalize(Path p) {
        return p.toAbsolutePath().normalize();
    }

    private static JsonArray strings(List<String> list) {
        JsonArray out = new JsonArray();
        list.forEach(out::add);
        return out;
    }

    private static List<String> strings(JsonObject o, String key) {
        List<String> out = new ArrayList<>();
        for (JsonElement e : array(o, key)) {
            if (!e.isJsonNull()) {
                out.add(e.getAsString());
            }
        }
        return out;
    }

    private static JsonArray array(JsonObject o, String key) {
        return o == null || !o.has(key) || !o.get(key).isJsonArray() ? new JsonArray() : o.getAsJsonArray(key);
    }

    static String string(JsonObject o, String key) {
        return o == null || !o.has(key) || o.get(key).isJsonNull() ? null : o.get(key).getAsString();
    }

    static JsonObject object(JsonObject o, String key) {
        return o == null || !o.has(key) || !o.get(key).isJsonObject() ? null : o.getAsJsonObject(key);
    }
}
