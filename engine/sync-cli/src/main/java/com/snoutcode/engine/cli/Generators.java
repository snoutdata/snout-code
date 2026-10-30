package com.snoutcode.engine.cli;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import org.gradle.tooling.BuildLauncher;
import org.gradle.tooling.ProjectConnection;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStreamReader;
import java.io.OutputStream;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Collection;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.stream.Stream;

/**
 * Source generators: the Gradle tasks that make what a compile needs and a sync leaves missing
 * (the sync plugin lists them per source set as {@code generators}: Kafka's message classes, an
 * ANTLR parser, a repackaged jar on the class path). Without them the editor would report every
 * use of the generated code as an error in correct code.
 *
 * <p>A Maven module's are the plugins bound to {@code generate-sources} or {@code process-sources}
 * (Modello, ANTLR, JAXB, protobuf...; see {@link MavenSync#generator}).
 *
 * <p>After the sync, the engine runs them, one Gradle build (or one Maven run) per build that has
 * any, with the build directories where the sync put them, while jdtls starts; the modules of
 * that build wait in {@code generatingSources}, and jdtls imports the projects again once the
 * code is there (a Maven build syncs again first, to find the folders its generators filled). A
 * generator that fails leaves the folder missing, as before, and says why in the log.
 */
final class Generators {
    /** One Gradle build's generators, and its modules, which wait for them. */
    record Job(ProjectSync.Plan plan, List<String> tasks, List<String> modules) {
    }

    private Generators() {
    }

    /** The generators of each synced Gradle build of the model, as tasks to run from its root. */
    static List<Job> jobs(JsonObject model, Map<String, ProjectSync.Plan> plans) {
        List<Job> out = new ArrayList<>();
        for (JsonElement g : array(model, "gradleBuilds")) {
            JsonObject gradleBuild = g.getAsJsonObject();
            ProjectSync.Plan plan = plans.get(string(gradleBuild, "id"));
            if (plan == null || !"synced".equals(string(gradleBuild, "status"))) {
                continue;
            }
            Set<String> tasks = new LinkedHashSet<>();
            if (plan.maven()) {
                // The modules whose plugins generate sources, as -pl selectors; skipped when a
                // run already filled their target/generated-sources.
                for (JsonElement b : array(gradleBuild, "builds")) {
                    for (JsonElement p : array(b.getAsJsonObject(), "projects")) {
                        JsonObject project = p.getAsJsonObject();
                        boolean generates = false;
                        for (JsonElement s : array(project, "sourceSets")) {
                            generates |= array(s.getAsJsonObject(), "generators").iterator().hasNext();
                        }
                        String dir = string(project, "projectDir");
                        if (generates && dir != null && !filled(Path.of(dir, "target", "generated-sources"))) {
                            tasks.add(string(project, "path"));
                        }
                    }
                }
            }
            for (JsonElement b : plan.maven() ? List.<JsonElement>of() : array(gradleBuild, "builds")) {
                JsonObject build = b.getAsJsonObject();
                String kind = string(build, "kind");
                String prefix = "root".equals(kind) ? "" : "included".equals(kind) ? ":" + string(build, "name") : null;
                if (prefix == null) {
                    continue; // buildSrc builds itself before the root build configures
                }
                for (JsonElement p : array(build, "projects")) {
                    for (JsonElement s : array(p.getAsJsonObject(), "sourceSets")) {
                        for (JsonElement t : array(s.getAsJsonObject(), "generators")) {
                            tasks.add(prefix + t.getAsString());
                        }
                    }
                }
            }
            if (tasks.isEmpty()) {
                continue;
            }
            List<String> modules = new ArrayList<>();
            for (JsonElement m : array(model, "modules")) {
                JsonObject module = m.getAsJsonObject();
                if (plan.id().equals(string(module, "build")) && string(module, "id") != null) {
                    modules.add(string(module, "id"));
                }
            }
            out.add(new Job(plan, new ArrayList<>(tasks), modules));
        }
        return out;
    }

    /** Whether a folder holds anything (a generator ran into it). */
    static boolean filled(Path dir) {
        try (Stream<Path> list = Files.list(dir)) {
            return list.findAny().isPresent();
        } catch (IOException e) {
            return false;
        }
    }

    /**
     * Runs a build's generators: Gradle's tasks with {@code --continue}, Maven's modules through
     * {@code process-sources} with {@code --fail-at-end}, so one that fails does not stop the
     * others. Null when they ran, else what went wrong.
     */
    static String run(Job job, OutputStream output) {
        return job.plan().maven() ? runMaven(job, output) : runGradle(job, output);
    }

    private static String runMaven(Job job, OutputStream output) {
        MavenRuns.Invocation inv = new MavenRuns.Invocation();
        inv.plan = job.plan();
        PrintStream progress = new PrintStream(output == null ? OutputStream.nullOutputStream() : output, true, StandardCharsets.UTF_8);
        try {
            List<String> command = MavenRuns.base(inv, progress);
            if (!job.tasks().contains(":")) {
                command.add("-pl");
                command.add(String.join(",", job.tasks()));
            }
            command.add("--fail-at-end");
            command.add("process-sources");
            ProcessBuilder builder = new ProcessBuilder(command).directory(job.plan().rootDir()).redirectErrorStream(true);
            if (job.plan().options().javaHome != null) {
                builder.environment().put("JAVA_HOME", job.plan().options().javaHome.getAbsolutePath());
            }
            String home = MavenEnv.userHome().toString();
            if (!home.equals(System.getProperty("user.home")) && MavenEnv.optsProperty("user.home") == null) {
                String opts = builder.environment().getOrDefault("MAVEN_OPTS", "");
                builder.environment().put("MAVEN_OPTS", (opts + " -Duser.home=" + home).trim());
            }
            Process process = builder.start();
            process.getOutputStream().close();
            List<String> errors = new ArrayList<>();
            try (BufferedReader reader = new BufferedReader(new InputStreamReader(process.getInputStream(), StandardCharsets.UTF_8))) {
                for (String line; (line = reader.readLine()) != null; ) {
                    progress.println(line);
                    if (line.startsWith("[ERROR]") && errors.size() < 3) {
                        errors.add(line.substring(7).trim());
                    }
                }
            }
            int exit = process.waitFor();
            return exit == 0 ? null : errors.isEmpty() ? "Maven exited with " + exit : String.join(" ", errors);
        } catch (IOException e) {
            return SnoutEngine.describe(e);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            return "interrupted";
        }
    }

    private static String runGradle(Job job, OutputStream output) {
        Sync.Options options = job.plan().options();
        try (ProjectConnection connection = Sync.connector(job.plan().rootDir(), options).connect()) {
            List<Path> scripts = new ArrayList<>();
            if (options.buildRoot != null) {
                scripts.add(Sync.buildDirInitScript(options.buildRoot));
            }
            List<String> args = Sync.arguments(scripts, options.projectCacheDir, options);
            args.add("--continue");
            BuildLauncher launcher = connection.newBuild().forTasks(job.tasks().toArray(new String[0])).withArguments(args);
            if (options.javaHome != null) {
                launcher.setJavaHome(options.javaHome);
            }
            OutputStream out = output == null ? OutputStream.nullOutputStream() : output;
            GradleSaid.Tail stderr = new GradleSaid.Tail(output);
            launcher.setStandardOutput(out);
            launcher.setStandardError(stderr);
            try {
                launcher.run();
                return null;
            } catch (RuntimeException e) {
                String said = GradleSaid.whatWentWrong(stderr.text());
                return said != null ? said : SnoutEngine.describe(e);
            }
        } catch (IOException | RuntimeException e) {
            return SnoutEngine.describe(e);
        }
    }

    /** Runs every job, one after another, and logs each. */
    static void runAll(Collection<Job> jobs, PrintStream log, boolean verbose) {
        for (Job job : jobs) {
            long start = System.nanoTime();
            String error = run(job, verbose ? log : null);
            long millis = (System.nanoTime() - start) / 1_000_000;
            String first = error == null ? null : error.strip().lines().findFirst().orElse(error);
            log.println("snout-engine: " + (error == null ? "generated sources" : "could not generate all sources") + " in "
                + job.plan().id() + " (" + String.join(" ", job.tasks()) + ") in " + millis + " ms"
                + (first == null ? "" : ": " + first));
        }
    }

    private static Iterable<JsonElement> array(JsonObject o, String key) {
        JsonElement e = o == null ? null : o.get(key);
        return e != null && e.isJsonArray() ? e.getAsJsonArray() : List.of();
    }

    private static String string(JsonObject o, String key) {
        JsonElement e = o == null ? null : o.get(key);
        return e == null || e.isJsonNull() ? null : e.getAsString();
    }
}
