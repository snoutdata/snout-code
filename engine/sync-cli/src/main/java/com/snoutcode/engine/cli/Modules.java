package com.snoutcode.engine.cli;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/** Finds a module (one Gradle project of a build) in a model by its id, or by a file it compiles. */
final class Modules {
    private Modules() {
    }

    /**
     * A module: its id as {@link ProjectModel} forms it ({@code <build>[/<included>][<path>]}),
     * its Gradle build entry and its project model.
     */
    record Ref(String id, String buildId, String includedBuild, String gradlePath, JsonObject gradleBuild, JsonObject project) {
        /** A task path of this module's build as the root build runs it: included builds get their prefix. */
        String taskPath(String pathInBuild) {
            return includedBuild == null ? pathInBuild : ":" + includedBuild + pathInBuild;
        }

        /** The test tasks, as the root build runs them. */
        List<String> testTasks() {
            List<String> out = new ArrayList<>();
            JsonArray tasks = Engine.array(project, "testTasks");
            if (tasks != null) {
                for (JsonElement t : tasks) {
                    out.add(taskPath(Engine.string(t.getAsJsonObject(), "path")));
                }
            }
            return out;
        }

        Path projectDir() {
            return Path.of(Engine.string(project, "projectDir"));
        }

        List<JsonObject> sourceSets() {
            List<JsonObject> out = new ArrayList<>();
            JsonArray sets = Engine.array(project, "sourceSets");
            if (sets != null) {
                sets.forEach(s -> out.add(s.getAsJsonObject()));
            }
            return out;
        }

        /** The source sets that test tasks run, or that are named as tests are. */
        boolean isTestSourceSet(String name) {
            JsonArray tasks = Engine.array(project, "testTasks");
            if (tasks != null) {
                for (JsonElement t : tasks) {
                    JsonArray sets = Engine.array(t.getAsJsonObject(), "sourceSets");
                    if (sets != null) {
                        for (JsonElement s : sets) {
                            if (s.getAsString().equals(name)) {
                                return true;
                            }
                        }
                    }
                }
            }
            return name.equals("test") || name.endsWith("Test") || name.endsWith("Tests");
        }
    }

    /** Every module of the model, in model order. */
    static List<Ref> all(JsonObject model) {
        List<Ref> out = new ArrayList<>();
        JsonArray builds = Engine.array(model, "gradleBuilds");
        if (builds == null) {
            return out;
        }
        for (JsonElement g : builds) {
            JsonObject gradleBuild = g.getAsJsonObject();
            String buildId = Engine.string(gradleBuild, "id");
            JsonArray inner = Engine.array(gradleBuild, "builds");
            if (inner == null) {
                continue;
            }
            for (JsonElement b : inner) {
                JsonObject build = b.getAsJsonObject();
                String kind = Engine.string(build, "kind");
                if ("buildSrc".equals(kind)) {
                    continue;
                }
                String included = "root".equals(kind) ? null : Engine.string(build, "name");
                String prefix = included == null ? buildId : buildId + "/" + included;
                for (JsonElement p : Engine.array(build, "projects")) {
                    JsonObject project = p.getAsJsonObject();
                    String path = Engine.string(project, "path");
                    String id = path == null || ":".equals(path) ? prefix : prefix + path;
                    out.add(new Ref(id, buildId, included, path, gradleBuild, project));
                }
            }
        }
        return out;
    }

    static Ref find(JsonObject model, String id) {
        for (Ref ref : all(model)) {
            if (ref.id().equals(id)) {
                return ref;
            }
        }
        return null;
    }

    /**
     * The module that compiles a file: the one with the deepest source root holding it, a test
     * source set's module first when {@code preferTest}; null when no module compiles it.
     */
    static Ref owner(JsonObject model, Path file, boolean preferTest) {
        Path target = file.toAbsolutePath().normalize();
        Ref best = null;
        int bestDepth = -1;
        boolean bestTest = false;
        for (Ref ref : all(model)) {
            for (JsonObject set : ref.sourceSets()) {
                boolean test = ref.isTestSourceSet(Engine.string(set, "name"));
                for (Path root : sourceDirs(set)) {
                    Path r = root.toAbsolutePath().normalize();
                    if (!target.startsWith(r)) {
                        continue;
                    }
                    int depth = r.getNameCount();
                    boolean better = depth > bestDepth || (depth == bestDepth && preferTest && test && !bestTest);
                    if (better) {
                        best = ref;
                        bestDepth = depth;
                        bestTest = test;
                    }
                }
            }
        }
        return best;
    }

    /** The source directories of a source set, every language. */
    static List<Path> sourceDirs(JsonObject sourceSet) {
        List<Path> out = new ArrayList<>();
        JsonObject sources = sourceSet.has("sources") && sourceSet.get("sources").isJsonObject()
            ? sourceSet.getAsJsonObject("sources") : new JsonObject();
        for (String language : new String[] {"java", "scala", "kotlin", "groovy"}) {
            JsonObject lang = sources.has(language) && sources.get(language).isJsonObject() ? sources.getAsJsonObject(language) : null;
            JsonArray dirs = Engine.array(lang, "srcDirs");
            if (dirs != null) {
                dirs.forEach(d -> out.add(Path.of(d.getAsString())));
            }
        }
        return out;
    }

    /** The resource directories of a source set. */
    static List<Path> resourceDirs(JsonObject sourceSet) {
        List<Path> out = new ArrayList<>();
        JsonObject sources = sourceSet.has("sources") && sourceSet.get("sources").isJsonObject()
            ? sourceSet.getAsJsonObject("sources") : new JsonObject();
        JsonObject resources = sources.has("resources") && sources.get("resources").isJsonObject() ? sources.getAsJsonObject("resources") : null;
        JsonArray dirs = Engine.array(resources, "srcDirs");
        if (dirs != null) {
            dirs.forEach(d -> out.add(Path.of(d.getAsString())));
        }
        return out;
    }
}
