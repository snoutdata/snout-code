package com.snoutcode.engine.cli;

import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

import java.io.File;
import java.io.IOException;
import java.nio.file.FileVisitResult;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.SimpleFileVisitor;
import java.nio.file.StandardCopyOption;
import java.nio.file.attribute.BasicFileAttributes;
import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TreeMap;
import java.util.regex.Pattern;

/**
 * The synced model, kept across starts with the inputs it came from, so a start whose builds are
 * unchanged skips the Gradle sync.
 *
 * <p>The inputs: every script the sync saw each build load (settings, build and applied scripts,
 * version catalogs, the wrapper's properties), each build's and included build's
 * {@code gradle.properties}, settings and build scripts (whether or not they exist, so a new one
 * counts), its {@code buildSrc} folder, the Gradle user home's {@code gradle.properties} and init
 * scripts; the builds the folder holds, each with its Gradle version and JDK (a newly installed
 * JDK may change one); and the engine's own options and sync plugin. Files are compared by
 * content, {@code buildSrc} by its files' sizes and times.
 *
 * <p>What a sync cannot promise to give again is <i>uncertain</i>: a build that failed for a
 * reason that may pass (the network, a daemon), dynamic or snapshot versions, libraries no longer
 * on disk. Those builds sync again in the background after a start from the cache.
 */
final class ModelCache {
    static final String VERSION = "1";

    private ModelCache() {
    }

    /** The record of a model's inputs, to write next to it. */
    static JsonObject inputs(JsonObject model, List<ProjectSync.Plan> plans, String optionsKey, File gradleUserHome) {
        JsonObject out = new JsonObject();
        out.addProperty("version", VERSION);
        out.addProperty("options", optionsKey);
        out.add("plans", plans(plans));
        JsonObject files = new JsonObject();
        for (Map.Entry<String, String> e : hashes(files(model, plans, gradleUserHome)).entrySet()) {
            files.addProperty(e.getKey(), e.getValue());
        }
        out.add("files", files);
        return out;
    }

    /**
     * Why the recorded inputs no longer hold, or null when they do and the model can be used as
     * it is.
     */
    static String changed(JsonObject recorded, List<ProjectSync.Plan> plans, String optionsKey) {
        if (recorded == null || !VERSION.equals(string(recorded, "version"))) {
            return "no record of the last sync";
        }
        if (!optionsKey.equals(string(recorded, "options"))) {
            return "the engine or its options changed";
        }
        if (!plans(plans).equals(recorded.get("plans"))) {
            return "the builds or their JDKs changed";
        }
        JsonObject files = recorded.has("files") ? recorded.getAsJsonObject("files") : new JsonObject();
        for (Map.Entry<String, JsonElement> e : files.entrySet()) {
            String now = hash(e.getKey());
            if (!now.equals(e.getValue().getAsString())) {
                return e.getKey() + " changed";
            }
        }
        return null;
    }

    static JsonObject read(Path file) {
        try {
            return Files.isRegularFile(file) ? JsonParser.parseString(Files.readString(file)).getAsJsonObject() : null;
        } catch (IOException | RuntimeException e) {
            return null;
        }
    }

    static void write(Path file, JsonObject inputs) throws IOException {
        Path temp = Files.createTempFile(file.getParent(), file.getFileName().toString(), ".tmp");
        Files.writeString(temp, new GsonBuilder().setPrettyPrinting().disableHtmlEscaping().create().toJson(inputs));
        Files.move(temp, file, StandardCopyOption.REPLACE_EXISTING, StandardCopyOption.ATOMIC_MOVE);
    }

    private static JsonArray plans(List<ProjectSync.Plan> plans) {
        JsonArray out = new JsonArray();
        for (ProjectSync.Plan p : plans) {
            out.add(p.id() + "|" + p.rootDir().getAbsolutePath() + "|" + p.gradleVersion() + "|"
                + (p.options().javaHome == null ? "" : p.options().javaHome.getAbsolutePath()) + "|" + p.needsJdk());
        }
        return out;
    }

    /** The files a model's sync read, as paths; a folder's path ends in {@code /**}. */
    static Set<String> files(JsonObject model, List<ProjectSync.Plan> plans, File gradleUserHome) {
        Set<String> out = new LinkedHashSet<>();
        for (JsonElement g : array(model, "gradleScripts")) {
            for (JsonElement f : array(g.getAsJsonObject(), "files")) {
                String path = string(f.getAsJsonObject(), "path");
                if (path != null) {
                    out.add(path);
                }
            }
        }
        Set<String> roots = new LinkedHashSet<>();
        plans.forEach(p -> roots.add(p.rootDir().getAbsolutePath()));
        for (JsonElement g : array(model, "gradleBuilds")) {
            JsonObject entry = g.getAsJsonObject();
            for (JsonElement b : array(entry, "builds")) {
                JsonObject build = b.getAsJsonObject();
                if (string(build, "rootDir") != null && !"buildSrc".equals(string(build, "kind"))) {
                    roots.add(string(build, "rootDir"));
                }
                for (JsonElement p : array(build, "projects")) {
                    JsonObject buildInfo = object(p.getAsJsonObject(), "build");
                    for (JsonElement t : array(buildInfo, "resyncTriggers")) {
                        String path = t.getAsString();
                        // Our own init scripts live in the temp folder, named by their content.
                        if (!path.contains(File.separator + "snout-engine" + File.separator)) {
                            out.add(path);
                        }
                    }
                }
            }
        }
        for (String root : roots) {
            for (String name : List.of("settings.gradle", "settings.gradle.kts", "build.gradle", "build.gradle.kts",
                "gradle.properties", "gradle/wrapper/gradle-wrapper.properties", "gradle/libs.versions.toml")) {
                out.add(new File(root, name).getAbsolutePath());
            }
            out.add(new File(root, "buildSrc").getAbsolutePath() + "/**");
        }
        File userHome = gradleUserHome != null ? gradleUserHome : new File(System.getProperty("user.home"), ".gradle");
        out.add(new File(userHome, "gradle.properties").getAbsolutePath());
        out.add(new File(userHome, "init.gradle").getAbsolutePath());
        out.add(new File(userHome, "init.gradle.kts").getAbsolutePath());
        out.add(new File(userHome, "init.d").getAbsolutePath() + "/**");
        return out;
    }

    private static Map<String, String> hashes(Set<String> files) {
        Map<String, String> out = new TreeMap<>();
        files.forEach(f -> out.put(f, hash(f)));
        return out;
    }

    /** A file's content hash, a folder's ({@code /**}) listing hash, or {@code missing}. */
    static String hash(String path) {
        try {
            if (path.endsWith("/**")) {
                Path dir = Path.of(path.substring(0, path.length() - 3));
                if (!Files.isDirectory(dir)) {
                    return "missing";
                }
                StringBuilder s = new StringBuilder();
                List<String> lines = new ArrayList<>();
                Files.walkFileTree(dir, new SimpleFileVisitor<>() {
                    @Override
                    public FileVisitResult preVisitDirectory(Path d, BasicFileAttributes attrs) {
                        String name = d.getFileName().toString();
                        return !d.equals(dir) && (name.equals("build") || name.equals(".gradle")) ? FileVisitResult.SKIP_SUBTREE
                            : FileVisitResult.CONTINUE;
                    }

                    @Override
                    public FileVisitResult visitFile(Path f, BasicFileAttributes attrs) {
                        lines.add(dir.relativize(f) + " " + attrs.size() + " " + attrs.lastModifiedTime().toMillis());
                        return FileVisitResult.CONTINUE;
                    }
                });
                lines.sort(null);
                lines.forEach(l -> s.append(l).append('\n'));
                return Sync.sha256(s.toString().getBytes(java.nio.charset.StandardCharsets.UTF_8));
            }
            Path file = Path.of(path);
            return Files.isRegularFile(file) ? Sync.sha256(Files.readAllBytes(file)) : "missing";
        } catch (IOException | RuntimeException e) {
            return "unreadable";
        }
    }

    // ---- what a sync may give differently

    private static final Pattern TRANSIENT = Pattern.compile(
        "Could not download|Could not GET|Could not HEAD|Connection refused|Connection reset|connect timed out|Read timed out"
            + "|No cached version|offline mode|daemon disappeared|UnknownHost|Could not install Gradle distribution");

    /**
     * The builds to sync again in the background after a start from the cache: those that failed
     * for a reason that may pass, and those with dynamic or snapshot versions or libraries no
     * longer on disk. A build that failed for a reason that holds as long as its inputs do (a
     * script error, a JDK it needs) is not.
     */
    static Set<String> uncertain(JsonObject model) {
        Set<String> out = new LinkedHashSet<>();
        for (JsonElement g : array(model, "gradleBuilds")) {
            JsonObject entry = g.getAsJsonObject();
            String id = string(entry, "id");
            String status = string(entry, "status");
            String error = string(entry, "error");
            if (("failed".equals(status) || "stale".equals(status)) && !entry.has("needsJdk")
                && (error == null || TRANSIENT.matcher(error).find())) {
                out.add(id);
                continue;
            }
            if (dynamic(entry)) {
                out.add(id);
            }
        }
        return out;
    }

    private static boolean dynamic(JsonObject entry) {
        for (JsonElement b : array(entry, "builds")) {
            for (JsonElement p : array(b.getAsJsonObject(), "projects")) {
                for (JsonElement s : array(p.getAsJsonObject(), "sourceSets")) {
                    for (String key : List.of("compileClasspath", "runtimeClasspath")) {
                        for (JsonElement e : array(s.getAsJsonObject(), key)) {
                            JsonObject c = e.getAsJsonObject();
                            String version = string(c, "version");
                            if (version != null && (version.contains("+") || version.contains("SNAPSHOT") || version.startsWith("latest."))) {
                                return true;
                            }
                            String file = string(c, "file");
                            if (file != null && "module".equals(string(c, "kind")) && !new File(file).exists()) {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        return false;
    }

    private static String string(JsonObject o, String key) {
        return o == null || !o.has(key) || o.get(key).isJsonNull() ? null : o.get(key).getAsString();
    }

    private static JsonObject object(JsonObject o, String key) {
        return o == null || !o.has(key) || !o.get(key).isJsonObject() ? null : o.getAsJsonObject(key);
    }

    private static JsonArray array(JsonObject o, String key) {
        return o == null || !o.has(key) || !o.get(key).isJsonArray() ? new JsonArray() : o.getAsJsonArray(key);
    }
}
