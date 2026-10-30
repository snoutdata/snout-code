package com.snoutcode.engine.cli;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

import java.io.IOException;
import java.io.UncheckedIOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import java.util.stream.Stream;

/**
 * The run configurations a model implies, per module:
 *
 * <ul>
 *   <li>a Spring Boot application: its main class, launched directly ({@code kind: main},
 *       {@code springBoot: true}), and its {@code bootRun} task ({@code kind: gradle});</li>
 *   <li>an {@code application} plugin: its main class, and its {@code run} task;</li>
 *   <li>every other class with a {@code main} method in a main (not test) source set, Java or
 *       Scala ({@code object} with {@code def main(args: Array[String])}, or {@code extends App});</li>
 *   <li>each test task ({@code kind: tests});</li>
 *   <li>in a Maven module with Spring Boot's plugin, {@code spring-boot:run} ({@code kind: maven}).</li>
 * </ul>
 */
final class RunConfigurations {
    private static final Pattern PACKAGE = Pattern.compile("^\\s*package\\s+([\\w.]+)", Pattern.MULTILINE);
    private static final Pattern JAVA_MAIN = Pattern.compile(
        "\\bstatic\\s+(?:(?:public|final|synchronized|strictfp)\\s+)*void\\s+main\\s*\\(\\s*(?:final\\s+)?String\\s*(?:\\[\\s*]|\\.\\.\\.)");
    private static final Pattern JAVA_MAIN_PUBLIC_FIRST = Pattern.compile(
        "\\bpublic\\s+(?:(?:final|synchronized|strictfp)\\s+)*static\\s+(?:(?:final|synchronized|strictfp)\\s+)*void\\s+main\\s*\\(\\s*(?:final\\s+)?String\\s*(?:\\[\\s*]|\\.\\.\\.)");
    private static final Pattern SCALA_MAIN = Pattern.compile("\\bdef\\s+main\\s*\\(\\s*\\w+\\s*:\\s*Array\\s*\\[\\s*String\\s*]\\s*\\)");
    private static final Pattern SCALA_OBJECT = Pattern.compile("\\bobject\\s+(\\w+)");
    private static final Pattern SCALA_APP = Pattern.compile("\\bobject\\s+(\\w+)\\s+extends\\s+App\\b");
    private static final int MAX_FILES = 20000;

    private RunConfigurations() {
    }

    /** Every configuration, or only a module's. */
    static JsonArray derive(JsonObject model, String onlyModule) {
        JsonArray out = new JsonArray();
        for (Modules.Ref module : Modules.all(model)) {
            if (onlyModule != null && !onlyModule.equals(module.id())) {
                continue;
            }
            derive(module, out);
        }
        return out;
    }

    private static void derive(Modules.Ref module, JsonArray out) {
        Set<String> mains = new LinkedHashSet<>();
        JsonObject boot = object(module.project(), "springBoot");
        if (boot != null) {
            String mainClass = firstNonNull(Engine.string(boot, "mainClass"),
                Engine.string(object(boot, "bootRun"), "mainClass"),
                Engine.string(boot, "bootJarMainClass"),
                Engine.string(boot, "jarManifestMainClass"),
                first(Engine.array(boot, "mainClassCandidates")));
            if (mainClass != null) {
                JsonObject c = main(module, mainClass);
                c.addProperty("springBoot", true);
                JsonObject run = object(boot, "bootRun");
                if (run != null) {
                    c.add("jvmArgs", run.has("jvmArgs") ? run.get("jvmArgs") : new JsonArray());
                    c.add("args", run.has("args") ? run.get("args") : new JsonArray());
                    if (Engine.string(run, "workingDir") != null) {
                        c.addProperty("workingDir", Engine.string(run, "workingDir"));
                    }
                }
                out.add(c);
                mains.add(mainClass);
            }
            JsonObject mavenRun = object(boot, "run");
            if (mavenRun != null && Engine.string(mavenRun, "goal") != null) {
                JsonObject g = base(module, "maven", "maven:" + module.id() + ":" + Engine.string(mavenRun, "goal"));
                g.addProperty("name", module.project().get("name").getAsString() + ": " + Engine.string(mavenRun, "goal"));
                g.addProperty("goals", Engine.string(mavenRun, "goal"));
                g.addProperty("springBoot", true);
                out.add(g);
            }
            JsonObject run = object(boot, "bootRun");
            if (run != null && Engine.string(run, "task") != null) {
                JsonObject g = gradle(module, module.taskPath(Engine.string(run, "task")));
                g.addProperty("springBoot", true);
                out.add(g);
            }
        }
        JsonObject application = object(module.project(), "application");
        if (application != null) {
            String mainClass = Engine.string(application, "mainClass");
            if (mainClass != null && mains.add(mainClass)) {
                JsonObject c = main(module, mainClass);
                c.add("jvmArgs", application.has("jvmArgs") ? application.get("jvmArgs") : new JsonArray());
                out.add(c);
            }
            if (Engine.string(application, "runTask") != null) {
                out.add(gradle(module, module.taskPath(Engine.string(application, "runTask"))));
            }
        }
        for (String mainClass : scanMains(module)) {
            if (mains.add(mainClass)) {
                out.add(main(module, mainClass));
            }
        }
        for (String task : module.testTasks()) {
            JsonObject c = base(module, "tests", "tests:" + module.id() + ":" + task);
            c.addProperty("name", module.id() + ": " + task.substring(task.lastIndexOf(':') + 1));
            c.addProperty("task", task);
            out.add(c);
        }
    }

    private static JsonObject main(Modules.Ref module, String mainClass) {
        JsonObject c = base(module, "main", "main:" + module.id() + ":" + mainClass);
        c.addProperty("name", mainClass.substring(mainClass.lastIndexOf('.') + 1));
        c.addProperty("mainClass", mainClass);
        c.addProperty("workingDir", module.projectDir().toString());
        c.add("args", new JsonArray());
        c.add("jvmArgs", new JsonArray());
        return c;
    }

    private static JsonObject gradle(Modules.Ref module, String task) {
        JsonObject c = base(module, "gradle", "gradle:" + module.id() + ":" + task);
        c.addProperty("name", module.id() + ": " + task.substring(task.lastIndexOf(':') + 1));
        c.addProperty("task", task);
        return c;
    }

    private static JsonObject base(Modules.Ref module, String kind, String id) {
        JsonObject c = new JsonObject();
        c.addProperty("id", id);
        c.addProperty("kind", kind);
        c.addProperty("module", module.id());
        c.addProperty("build", module.buildId());
        return c;
    }

    /** The classes with a main method in the module's main source sets, sorted. */
    static List<String> scanMains(Modules.Ref module) {
        Set<String> found = new java.util.TreeSet<>();
        int[] budget = {MAX_FILES};
        for (JsonObject set : module.sourceSets()) {
            if (module.isTestSourceSet(Engine.string(set, "name"))) {
                continue;
            }
            for (Path dir : Modules.sourceDirs(set)) {
                if (!Files.isDirectory(dir)) {
                    continue;
                }
                try (Stream<Path> walk = Files.walk(dir)) {
                    walk.filter(p -> {
                        String n = p.getFileName().toString();
                        return n.endsWith(".java") || n.endsWith(".scala");
                    }).limit(budget[0]).forEach(file -> {
                        budget[0]--;
                        found.addAll(mains(file));
                    });
                } catch (IOException e) {
                    // an unreadable folder has no mains for us
                }
            }
        }
        return new ArrayList<>(found);
    }

    /** The main classes a source file declares. */
    static List<String> mains(Path file) {
        String text;
        try {
            text = Files.readString(file);
        } catch (IOException | UncheckedIOException e) {
            return List.of();
        }
        return mains(file.getFileName().toString(), text);
    }

    static List<String> mains(String fileName, String text) {
        if (!text.contains("main") && !text.contains("App")) {
            return List.of();
        }
        Matcher pkg = PACKAGE.matcher(text);
        String prefix = pkg.find() ? pkg.group(1) + "." : "";
        List<String> out = new ArrayList<>();
        if (fileName.endsWith(".java")) {
            if (JAVA_MAIN.matcher(text).find() || JAVA_MAIN_PUBLIC_FIRST.matcher(text).find()) {
                out.add(prefix + fileName.substring(0, fileName.length() - ".java".length()));
            }
            return out;
        }
        Map<Integer, String> objects = new LinkedHashMap<>();
        Matcher o = SCALA_OBJECT.matcher(text);
        while (o.find()) {
            objects.put(o.start(), o.group(1));
        }
        Matcher m = SCALA_MAIN.matcher(text);
        while (m.find()) {
            String owner = null;
            for (Map.Entry<Integer, String> e : objects.entrySet()) {
                if (e.getKey() < m.start()) {
                    owner = e.getValue();
                }
            }
            if (owner != null && !out.contains(prefix + owner)) {
                out.add(prefix + owner);
            }
        }
        Matcher app = SCALA_APP.matcher(text);
        while (app.find()) {
            if (!out.contains(prefix + app.group(1))) {
                out.add(prefix + app.group(1));
            }
        }
        return out;
    }

    private static String firstNonNull(String... values) {
        for (String v : values) {
            if (v != null && !v.isBlank()) {
                return v;
            }
        }
        return null;
    }

    private static String first(JsonArray array) {
        return array == null || array.isEmpty() ? null : array.get(0).getAsString();
    }

    private static JsonObject object(JsonObject o, String key) {
        if (o == null) {
            return null;
        }
        JsonElement e = o.get(key);
        return e != null && e.isJsonObject() ? e.getAsJsonObject() : null;
    }
}
