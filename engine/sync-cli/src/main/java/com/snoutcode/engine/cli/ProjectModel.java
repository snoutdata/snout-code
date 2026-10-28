package com.snoutcode.engine.cli;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

import java.io.File;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

/**
 * The project view of a model: the sections an IDE shows, assembled from the Gradle builds'
 * raw models. {@link #assemble} adds to the model:
 *
 * <ul>
 *   <li>{@code modules}: one per Gradle project, with its content root, typed source roots,
 *       excluded folders, dependencies per scope and JDK;</li>
 *   <li>{@code sharedSourceRoots}: source roots that no module owns (they lie outside every
 *       module's content root, or in a module that does not compile them), with the modules that
 *       compile each;</li>
 *   <li>{@code libraries}: every jar of every module, once;</li>
 *   <li>{@code gradleScripts}: the build's scripts and properties files, per build;</li>
 *   <li>{@code failedBuilds}: the builds that did not sync, and why.</li>
 * </ul>
 */
final class ProjectModel {
    private ProjectModel() {
    }

    /** One Gradle project being turned into a module. */
    private static final class Mod {
        String id;
        JsonObject gradleBuild;
        JsonObject build;
        JsonObject project;
        Path contentRoot;
        final JsonObject out = new JsonObject();
    }

    static void assemble(JsonObject model) {
        File folder = new File(model.get("rootDir").getAsString());
        List<Mod> mods = new ArrayList<>();
        Map<String, Mod> byBuildKey = new HashMap<>();
        Map<String, Mod> byOutputDir = new HashMap<>();
        JsonArray failed = new JsonArray();
        JsonArray scripts = new JsonArray();

        for (JsonElement g : array(model, "gradleBuilds")) {
            JsonObject gradleBuild = g.getAsJsonObject();
            String buildId = string(gradleBuild, "id");
            scripts.add(scripts(gradleBuild));
            String status = string(gradleBuild, "status");
            if ("failed".equals(status) || "stale".equals(status)) {
                JsonObject f = new JsonObject();
                f.addProperty("build", buildId);
                f.addProperty("rootDir", string(gradleBuild, "rootDir"));
                f.addProperty("gradleVersion", string(gradleBuild, "gradleVersion"));
                f.addProperty("javaHome", string(gradleBuild, "gradleJavaHome"));
                f.addProperty("reason", string(gradleBuild, "reason"));
                if (gradleBuild.has("needsJdk")) {
                    f.add("needsJdk", gradleBuild.get("needsJdk"));
                }
                f.addProperty("error", string(gradleBuild, "error"));
                // A stale build failed now, but its modules come from the last sync that worked.
                f.addProperty("stale", "stale".equals(status));
                failed.add(f);
            }
            for (JsonElement b : array(gradleBuild, "builds")) {
                JsonObject build = b.getAsJsonObject();
                String kind = string(build, "kind");
                String prefix = "root".equals(kind) ? buildId : buildId + "/" + string(build, "name");
                for (JsonElement p : array(build, "projects")) {
                    JsonObject project = p.getAsJsonObject();
                    Mod mod = new Mod();
                    String path = string(project, "path");
                    mod.id = ":".equals(path) || path == null ? prefix : prefix + path;
                    mod.gradleBuild = gradleBuild;
                    mod.build = build;
                    mod.project = project;
                    mod.contentRoot = Path.of(string(project, "projectDir")).normalize();
                    mods.add(mod);
                    for (String key : buildKeys(build)) {
                        byBuildKey.put(buildId + "|" + key + "|" + path, mod);
                    }
                    for (JsonElement s : array(project, "sourceSets")) {
                        JsonObject output = object(s.getAsJsonObject(), "output");
                        for (String dir : strings(output, "classesDirs")) {
                            byOutputDir.put(dir, mod);
                        }
                        String resources = string(output, "resourcesDir");
                        if (resources != null) {
                            byOutputDir.put(resources, mod);
                        }
                    }
                }
            }
        }

        Map<String, JsonObject> libraries = new LinkedHashMap<>();
        Map<String, Set<String>> rootUsers = new LinkedHashMap<>();
        Map<String, JsonObject> rootInfo = new HashMap<>();
        for (Mod mod : mods) {
            mod.out.addProperty("id", mod.id);
            mod.out.addProperty("name", string(mod.project, "name"));
            mod.out.addProperty("build", string(mod.gradleBuild, "id"));
            mod.out.addProperty("buildKind", string(mod.build, "kind"));
            mod.out.addProperty("includedBuild", "root".equals(string(mod.build, "kind")) ? null : string(mod.build, "name"));
            mod.out.addProperty("gradlePath", string(mod.project, "path"));
            mod.out.addProperty("contentRoot", mod.contentRoot.toString());
            mod.out.addProperty("buildFile", string(mod.project, "buildFile"));
            JsonArray roots = sourceRoots(mod);
            mod.out.add("sourceRoots", roots);
            for (JsonElement r : roots) {
                JsonObject root = r.getAsJsonObject();
                if ("generated".equals(string(root, "type"))) {
                    continue; // a module's generated sources are its own, wherever its build folder is
                }
                String dir = string(root, "dir");
                rootUsers.computeIfAbsent(dir, k -> new LinkedHashSet<>()).add(mod.id);
                rootInfo.putIfAbsent(dir, root);
            }
            mod.out.add("excludedDirs", excluded(mod));
            mod.out.add("dependencies", dependencies(mod, byBuildKey, byOutputDir, libraries));
            mod.out.add("jdk", jdk(mod));
        }

        // Who owns each source root: the module with the deepest content root holding it, when
        // that module compiles it. A root nobody owns is shared.
        Map<String, String> owners = new HashMap<>();
        Map<String, Mod> byId = new HashMap<>();
        for (Mod mod : mods) {
            byId.put(mod.id, mod);
        }
        for (Map.Entry<String, Set<String>> e : rootUsers.entrySet()) {
            Path dir = Path.of(e.getKey());
            Mod deepest = null;
            for (Mod mod : mods) {
                if (dir.startsWith(mod.contentRoot)
                    && (deepest == null || mod.contentRoot.getNameCount() > deepest.contentRoot.getNameCount())) {
                    deepest = mod;
                }
            }
            if (deepest != null && e.getValue().contains(deepest.id)) {
                owners.put(e.getKey(), deepest.id);
            }
        }
        JsonArray shared = new JsonArray();
        Map<Path, String> sharedNames = new LinkedHashMap<>();
        Set<String> takenNames = new LinkedHashSet<>();
        for (Mod mod : mods) {
            takenNames.add(mod.id);
        }
        for (Map.Entry<String, Set<String>> e : rootUsers.entrySet()) {
            if (owners.containsKey(e.getKey())) {
                continue;
            }
            Path dir = Path.of(e.getKey());
            Path base = sharedBase(dir);
            String name = sharedNames.computeIfAbsent(base, b -> unique(takenNames, sharedName(folder.toPath(), b)));
            JsonObject info = rootInfo.get(e.getKey());
            JsonObject root = new JsonObject();
            root.addProperty("dir", e.getKey());
            root.addProperty("type", string(info, "type"));
            root.add("languages", info.get("languages"));
            root.addProperty("exists", Files.isDirectory(dir));
            root.addProperty("name", name);
            root.addProperty("contentRoot", base.toString());
            JsonArray users = new JsonArray();
            e.getValue().forEach(users::add);
            root.add("modules", users);
            shared.add(root);
        }
        for (Mod mod : mods) {
            for (JsonElement r : mod.out.getAsJsonArray("sourceRoots")) {
                JsonObject root = r.getAsJsonObject();
                if ("generated".equals(string(root, "type"))) {
                    root.addProperty("shared", false);
                    continue;
                }
                String owner = owners.get(string(root, "dir"));
                root.addProperty("shared", owner == null);
                if (owner != null && !owner.equals(mod.id)) {
                    root.addProperty("owner", owner);
                }
            }
        }

        JsonArray modules = new JsonArray();
        mods.forEach(m -> modules.add(m.out));
        JsonArray libs = new JsonArray();
        libraries.values().forEach(libs::add);
        model.add("modules", modules);
        model.add("sharedSourceRoots", shared);
        model.add("libraries", libs);
        model.add("gradleScripts", scripts);
        model.add("failedBuilds", failed);
    }

    // ---- source roots

    private static JsonArray sourceRoots(Mod mod) {
        Set<String> tests = testSourceSets(mod.project);
        Map<String, JsonObject> byDir = new LinkedHashMap<>();
        // Sources before resources: a folder that is both (resources read from src/main/java) is a source root.
        for (boolean resources : new boolean[] {false, true}) {
            for (JsonElement s : array(mod.project, "sourceSets")) {
                JsonObject sourceSet = s.getAsJsonObject();
                String name = string(sourceSet, "name");
                boolean test = tests.contains(name);
                for (JsonElement r : array(sourceSet, "roots")) {
                    JsonObject root = r.getAsJsonObject();
                    String kind = string(root, "kind");
                    if ("resource".equals(kind) != resources) {
                        continue;
                    }
                    String type = switch (String.valueOf(kind)) {
                        case "resource" -> test ? "test-resources" : "resources";
                        case "generated" -> "generated";
                        default -> test ? "test" : "main";
                    };
                    String dir = Path.of(string(root, "dir")).normalize().toString();
                    if (byDir.containsKey(dir)) {
                        continue;
                    }
                    JsonObject out = new JsonObject();
                    out.addProperty("dir", dir);
                    out.addProperty("type", type);
                    out.addProperty("sourceSet", name);
                    if ("generated".equals(kind)) {
                        out.addProperty("test", test);
                    }
                    out.add("languages", root.has("languages") ? root.get("languages") : new JsonArray());
                    out.addProperty("exists", root.has("exists") ? root.get("exists").getAsBoolean() : Files.isDirectory(Path.of(dir)));
                    byDir.put(dir, out);
                }
            }
        }
        JsonArray out = new JsonArray();
        byDir.values().forEach(out::add);
        return out;
    }

    /**
     * The folder a shared root belongs to, for naming and grouping: {@code x/src/main/java} and
     * {@code x/src/main/resources} both belong to {@code x}.
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

    /** A shared root group's name: its path inside the folder, or its own name outside it. */
    static String sharedName(Path folder, Path base) {
        if (base.startsWith(folder) && !base.equals(folder)) {
            return folder.relativize(base).toString().replace(File.separatorChar, '/');
        }
        return base.getFileName() == null ? "shared" : base.getFileName().toString();
    }

    private static String unique(Set<String> taken, String name) {
        String candidate = name;
        for (int i = 2; taken.contains(candidate); i++) {
            candidate = name + "-" + i;
        }
        taken.add(candidate);
        return candidate;
    }

    private static JsonArray excluded(Mod mod) {
        Set<String> dirs = new LinkedHashSet<>();
        dirs.add(mod.contentRoot.resolve("build").toString());
        if (":".equals(string(mod.project, "path"))) {
            dirs.add(mod.contentRoot.resolve(".gradle").toString());
        }
        String buildDir = string(mod.project, "buildDir");
        if (buildDir != null && Path.of(buildDir).startsWith(mod.contentRoot)) {
            dirs.add(Path.of(buildDir).normalize().toString());
        }
        for (JsonElement s : array(mod.project, "sourceSets")) {
            JsonObject output = object(s.getAsJsonObject(), "output");
            List<String> outputs = new ArrayList<>(strings(output, "classesDirs"));
            if (string(output, "resourcesDir") != null) {
                outputs.add(string(output, "resourcesDir"));
            }
            for (String o : outputs) {
                Path p = Path.of(o).normalize();
                if (p.startsWith(mod.contentRoot) && dirs.stream().noneMatch(d -> p.startsWith(Path.of(d)))) {
                    dirs.add(p.toString());
                }
            }
        }
        JsonArray out = new JsonArray();
        dirs.forEach(out::add);
        return out;
    }

    // ---- dependencies

    /**
     * Dependencies by scope, the way an IDE shows them: {@code compile} (compile and runtime),
     * {@code provided} (compile only), {@code runtime} (runtime only) for the main code, and
     * {@code test} for what only the tests use.
     */
    private static JsonObject dependencies(Mod mod, Map<String, Mod> byBuildKey, Map<String, Mod> byOutputDir,
                                           Map<String, JsonObject> libraries) {
        Set<String> tests = testSourceSets(mod.project);
        Map<String, JsonObject> refs = new LinkedHashMap<>();
        Set<String> mainCompile = new LinkedHashSet<>();
        Set<String> mainRuntime = new LinkedHashSet<>();
        Set<String> test = new LinkedHashSet<>();
        for (JsonElement s : array(mod.project, "sourceSets")) {
            JsonObject sourceSet = s.getAsJsonObject();
            boolean isTest = tests.contains(string(sourceSet, "name"));
            for (String classpath : new String[] {"compileClasspath", "runtimeClasspath"}) {
                Set<String> into = isTest ? test : "compileClasspath".equals(classpath) ? mainCompile : mainRuntime;
                for (JsonElement e : array(sourceSet, classpath)) {
                    for (JsonObject ref : resolve(mod, e.getAsJsonObject(), byBuildKey, byOutputDir, libraries)) {
                        String key = string(ref, "kind") + "|" + string(ref, "id");
                        refs.putIfAbsent(key, ref);
                        into.add(key);
                    }
                }
            }
        }
        JsonObject out = new JsonObject();
        out.add("compile", select(refs, k -> mainCompile.contains(k) && mainRuntime.contains(k)));
        out.add("provided", select(refs, k -> mainCompile.contains(k) && !mainRuntime.contains(k)));
        out.add("runtime", select(refs, k -> !mainCompile.contains(k) && mainRuntime.contains(k)));
        out.add("test", select(refs, k -> test.contains(k) && !mainCompile.contains(k) && !mainRuntime.contains(k)));
        return out;
    }

    private static JsonArray select(Map<String, JsonObject> refs, java.util.function.Predicate<String> keep) {
        JsonArray out = new JsonArray();
        refs.forEach((k, v) -> {
            if (keep.test(k)) {
                out.add(v);
            }
        });
        return out;
    }

    /** A classpath entry as references to modules and libraries; none for this module's own output. */
    private static List<JsonObject> resolve(Mod mod, JsonObject entry, Map<String, Mod> byBuildKey,
                                            Map<String, Mod> byOutputDir, Map<String, JsonObject> libraries) {
        List<JsonObject> out = new ArrayList<>();
        String kind = string(entry, "kind");
        String buildId = string(mod.gradleBuild, "id");
        if ("sourceSetOutput".equals(kind)) {
            return out;
        }
        if ("project".equals(kind)) {
            Mod target = byBuildKey.get(buildId + "|" + string(entry, "build") + "|" + string(entry, "path"));
            if (target == null) {
                for (String f : strings(entry, "files")) {
                    target = target != null ? target : byOutputDir.get(f);
                }
            }
            if (target != null) {
                if (target != mod) {
                    out.add(ref("module", target.id));
                }
            } else {
                for (String f : strings(entry, "files")) {
                    out.add(ref("library", library(libraries, null, null, null, f, null)));
                }
            }
            return out;
        }
        String file = string(entry, "file");
        if (file == null) {
            return out;
        }
        Mod owner = byOutputDir.get(file);
        if (owner != null) {
            if (owner != mod) {
                out.add(ref("module", owner.id));
            }
            return out;
        }
        out.add(ref("library", library(libraries, string(entry, "group"), string(entry, "name"),
            string(entry, "version"), file, string(entry, "sources"))));
        return out;
    }

    private static JsonObject ref(String kind, String id) {
        JsonObject o = new JsonObject();
        o.addProperty("kind", kind);
        o.addProperty("id", id);
        return o;
    }

    /** The library for a file, added once; returns its id. */
    private static String library(Map<String, JsonObject> libraries, String group, String name, String version,
                                  String file, String sources) {
        JsonObject existing = libraries.get(file);
        if (existing != null) {
            if (string(existing, "sources") == null && sources != null) {
                existing.addProperty("sources", sources);
            }
            return string(existing, "id");
        }
        String id = group != null && name != null ? group + ":" + name + (version == null ? "" : ":" + version) : file;
        String base = id;
        for (int i = 2; libraryIds(libraries).contains(id); i++) {
            id = base + " (" + new File(file).getName() + (i > 2 ? " " + i : "") + ")";
        }
        JsonObject lib = new JsonObject();
        lib.addProperty("id", id);
        lib.addProperty("group", group);
        lib.addProperty("name", name != null ? name : new File(file).getName());
        lib.addProperty("version", version);
        lib.addProperty("file", file);
        lib.addProperty("sources", sources);
        libraries.put(file, lib);
        return id;
    }

    private static Set<String> libraryIds(Map<String, JsonObject> libraries) {
        Set<String> ids = new LinkedHashSet<>();
        libraries.values().forEach(l -> ids.add(string(l, "id")));
        return ids;
    }

    // ---- JDK

    private static JsonObject jdk(Mod mod) {
        String javaHome = string(mod.gradleBuild, "gradleJavaHome");
        String level = string(object(mod.project, "java"), "sourceCompatibility");
        for (JsonElement s : array(mod.project, "sourceSets")) {
            JsonObject sourceSet = s.getAsJsonObject();
            if (!"main".equals(string(sourceSet, "name"))) {
                continue;
            }
            JsonObject compile = object(sourceSet, "compile");
            JsonObject javac = object(compile, "java");
            if (javac == null && compile != null) {
                for (String language : compile.keySet()) {
                    javac = object(compile, language);
                    break;
                }
            }
            if (javac != null) {
                String toolchain = string(object(javac, "toolchain"), "javaHome");
                javaHome = toolchain != null ? toolchain : javaHome;
                String release = string(javac, "release");
                level = release != null ? release : string(javac, "sourceCompatibility") != null ? string(javac, "sourceCompatibility") : level;
            }
        }
        JsonObject out = new JsonObject();
        out.addProperty("javaHome", javaHome);
        JavaHomes.Jdk jdk = javaHome == null ? null : JavaHomes.read(new File(javaHome));
        out.addProperty("version", jdk == null ? null : jdk.version());
        out.addProperty("languageLevel", level);
        return out;
    }

    // ---- Gradle scripts

    /** The files that define a build: settings, build scripts, applied scripts, properties, wrapper. */
    private static JsonObject scripts(JsonObject gradleBuild) {
        Map<String, String> files = new LinkedHashMap<>();
        Path root = Path.of(string(gradleBuild, "rootDir"));
        for (String name : Discovery.SETTINGS) {
            addFile(files, root.resolve(name), "settings");
        }
        for (JsonElement b : array(gradleBuild, "builds")) {
            for (JsonElement p : array(b.getAsJsonObject(), "projects")) {
                JsonObject project = p.getAsJsonObject();
                if (string(project, "buildFile") != null) {
                    addFile(files, Path.of(string(project, "buildFile")), "build");
                }
                JsonObject build = object(project, "build");
                if (build != null && string(build, "settingsFile") != null) {
                    addFile(files, Path.of(string(build, "settingsFile")), "settings");
                }
                for (String s : strings(object(project, "appliedScripts"), "scripts")) {
                    addFile(files, Path.of(s), "applied");
                }
                addFile(files, Path.of(string(project, "projectDir"), "gradle.properties"), "properties");
            }
        }
        for (String name : Discovery.BUILD_FILES) {
            addFile(files, root.resolve(name), "build");
        }
        addFile(files, root.resolve("gradle.properties"), "properties");
        addFile(files, root.resolve("gradle/libs.versions.toml"), "versionCatalog");
        addFile(files, root.resolve("gradle/wrapper/gradle-wrapper.properties"), "wrapper");
        JsonObject out = new JsonObject();
        out.addProperty("build", string(gradleBuild, "id"));
        JsonArray list = new JsonArray();
        files.forEach((path, kind) -> {
            JsonObject f = new JsonObject();
            f.addProperty("path", path);
            f.addProperty("kind", kind);
            list.add(f);
        });
        out.add("files", list);
        return out;
    }

    private static void addFile(Map<String, String> files, Path file, String kind) {
        if (Files.isRegularFile(file)) {
            files.putIfAbsent(file.toAbsolutePath().normalize().toString(), kind);
        }
    }

    // ---- helpers

    /** Source sets that a test task runs, plus {@code test} and names ending in Test or Tests. */
    static Set<String> testSourceSets(JsonObject project) {
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

    /** The names a project dependency may give its build: {@code :} or the included build's name. */
    private static List<String> buildKeys(JsonObject build) {
        List<String> keys = new ArrayList<>();
        if ("root".equals(string(build, "kind"))) {
            keys.add(":");
            return keys;
        }
        String dirName = new File(String.valueOf(string(build, "rootDir"))).getName();
        for (String name : new String[] {dirName, string(build, "name")}) {
            if (name != null && !name.isEmpty()) {
                keys.add(":" + name);
                keys.add(name);
            }
        }
        return keys;
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
