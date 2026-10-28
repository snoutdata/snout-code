package com.snoutcode.engine.cli;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

import java.io.File;
import java.io.IOException;
import java.io.OutputStream;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.List;

/** Command line entry point of snout-engine. */
public final class SnoutEngine {
    private static final String USAGE = String.join("\n",
        "usage: snout-engine sync <folder> [options]",
        "       snout-engine lsp <folder> [options]",
        "",
        "sync: finds the Gradle builds of the folder (the folder's own build, or every build under it),",
        "fetches each build's model (configuration only, no tasks run) and writes one project model as JSON.",
        "",
        "options:",
        "  --out <file>                write the model here instead of stdout",
        "  --java-home <dir>           preferred JDK for the Gradle daemons, used for every build whose",
        "                              Gradle runs on it (default: chosen per build from the JDKs found)",
        "  --jdk <dir>                 another JDK to choose from (repeatable)",
        "  --parallel <n>              builds synced at once (default: " + ProjectSync.defaultParallelism() + ")",
        "  --gradle-user-home <dir>    Gradle user home (default: ~/.gradle)",
        "  --project-cache-dir <dir>   keep Gradle's project caches out of the builds, one folder per build",
        "  --gradle-version <version>  use this Gradle version instead of the build's wrapper",
        "  --no-sources                do not look up source jars",
        "  --offline                   run Gradle offline",
        "  --verbose                   show Gradle's output on stderr",
        "  --daemon-idle-minutes <n>   stop the Gradle daemons after n minutes without a build (default: "
            + Daemons.IDLE_MINUTES + ")",
        "",
        "lsp: syncs the model, compiles the Scala that Java code uses, then runs jdtls with the SnoutCode",
        "importer and java-debug, and speaks LSP on stdin and stdout: jdtls's methods, and the engine's",
        "own snout/ methods (Gradle tasks, tests, run configurations, launching and debugging, Scala,",
        "Spring), and the application properties and YAML files.",
        "Nothing is written into the project directory.",
        "",
        "options: --java-home, --jdk, --parallel, --gradle-user-home, --gradle-version, --no-sources,",
        "--offline, --verbose and --daemon-idle-minutes as for sync, and:",
        "  --cache-dir <dir>           caches, models and jdtls workspaces (default: the user cache dir)",
        "  --jdtls-java-home <dir>     Java 21+ for jdtls (default: this JVM)",
        "  --jdtls <dir>               jdtls installation (default: the bundled one)",
        "  --bundle <jar>              importer bundle (default: the bundled one)",
        "  --debug-bundle <jar>        java-debug bundle (default: the bundled one)",
        "  --no-debugger               do not load java-debug into jdtls",
        "  --spring-bundle <jar>       Spring support bundle (default: the bundled one)",
        "  --no-spring                 no Spring support",
        "  --no-scala-bridge           do not compile Scala for jdtls",
        "  --max-heap <size>           jdtls heap limit (default: 2G)",
        "  --jvm-arg <arg>             an extra JVM argument for jdtls (repeatable)");

    private SnoutEngine() {
    }

    public static void main(String[] args) {
        // The engine works for the process that started it (the app): it ends with it.
        Lsp.exitWithParent(System.err);
        System.exit(run(args, System.out, System.err));
    }

    static int run(String[] args, PrintStream stdout, PrintStream stderr) {
        if (args.length >= 2 && "lsp".equals(args[0])) {
            return lsp(args, stderr);
        }
        if (args.length < 2 || !"sync".equals(args[0])) {
            stderr.println(USAGE);
            return 2;
        }
        ProjectSync.Options project = new ProjectSync.Options();
        Sync.Options options = project.sync;
        options.projectDir = new File(args[1]);
        project.cacheDir = Lsp.defaultCacheDir();
        File out = null;
        boolean verbose = false;
        try {
            for (int i = 2; i < args.length; i++) {
                switch (args[i]) {
                    case "--out" -> out = new File(args[++i]);
                    case "--jdk" -> project.jdks.add(new File(args[++i]));
                    case "--parallel" -> project.parallel = Integer.parseInt(args[++i]);
                    case "--java-home" -> options.javaHome = new File(args[++i]);
                    case "--gradle-user-home" -> options.gradleUserHome = new File(args[++i]);
                    case "--project-cache-dir" -> options.projectCacheDir = new File(args[++i]);
                    case "--gradle-version" -> options.gradleVersion = args[++i];
                    case "--no-sources" -> options.sources = false;
                    case "--offline" -> options.offline = true;
                    case "--verbose" -> verbose = true;
                    case "--daemon-idle-minutes" -> Daemons.idleMinutes(Integer.parseInt(args[++i]));
                    default -> {
                        stderr.println("unknown option: " + args[i] + "\n\n" + USAGE);
                        return 2;
                    }
                }
            }
        } catch (ArrayIndexOutOfBoundsException e) {
            stderr.println("missing value for " + args[args.length - 1] + "\n\n" + USAGE);
            return 2;
        } catch (NumberFormatException e) {
            stderr.println("not a number: " + e.getMessage() + "\n\n" + USAGE);
            return 2;
        }
        if (!options.projectDir.isDirectory()) {
            stderr.println("not a directory: " + options.projectDir);
            return 2;
        }
        options.gradleOutput = verbose ? stderr : OutputStream.nullOutputStream();
        project.log = verbose ? stderr : null;
        // The engine's own daemons, which the lsp command of the same folder then takes warm; they
        // stop after a short idle time (see Daemons).
        try {
            options.daemonRegistry = Daemons.registry(project.cacheDir, options.projectDir.getCanonicalFile());
        } catch (IOException e) {
            stderr.println("not a directory: " + options.projectDir);
            return 2;
        }

        try {
            JsonObject model = ProjectSync.run(project);
            Gson gson = new GsonBuilder().setPrettyPrinting().disableHtmlEscaping().serializeNulls().create();
            String json = gson.toJson(model);
            if (out == null) {
                stdout.println(json);
            } else {
                File parent = out.getAbsoluteFile().getParentFile();
                if (parent != null) {
                    Files.createDirectories(parent.toPath());
                }
                Files.writeString(out.toPath(), json + "\n", StandardCharsets.UTF_8);
            }
            stderr.println(summary(model));
            return ProjectSync.anyFailed(model) ? 1 : 0;
        } catch (Exception e) {
            stderr.println("sync failed: " + describe(e));
            return 1;
        }
    }

    private static int lsp(String[] args, PrintStream stderr) {
        Lsp.Options options = new Lsp.Options();
        options.projectDir = new File(args[1]);
        try {
            for (int i = 2; i < args.length; i++) {
                switch (args[i]) {
                    case "--java-home" -> options.javaHome = new File(args[++i]);
                    case "--jdk" -> options.jdks.add(new File(args[++i]));
                    case "--parallel" -> options.parallel = Integer.parseInt(args[++i]);
                    case "--gradle-user-home" -> options.gradleUserHome = new File(args[++i]);
                    case "--gradle-version" -> options.gradleVersion = args[++i];
                    case "--no-sources" -> options.sources = false;
                    case "--offline" -> options.offline = true;
                    case "--verbose" -> options.verbose = true;
                    case "--daemon-idle-minutes" -> Daemons.idleMinutes(Integer.parseInt(args[++i]));
                    case "--cache-dir" -> options.cacheDir = new File(args[++i]);
                    case "--jdtls-java-home" -> options.jdtlsJavaHome = new File(args[++i]);
                    case "--jdtls" -> options.jdtlsHome = new File(args[++i]);
                    case "--bundle" -> options.bundle = new File(args[++i]);
                    case "--debug-bundle" -> options.debugBundle = new File(args[++i]);
                    case "--no-debugger" -> options.debugger = false;
                    case "--spring-bundle" -> options.springBundle = new File(args[++i]);
                    case "--no-spring" -> options.spring = false;
                    case "--no-scala-bridge" -> options.scalaBridge = false;
                    case "--max-heap" -> options.maxHeap = args[++i];
                    case "--jvm-arg" -> options.jvmArgs.add(args[++i]);
                    default -> {
                        stderr.println("unknown option: " + args[i] + "\n\n" + USAGE);
                        return 2;
                    }
                }
            }
        } catch (ArrayIndexOutOfBoundsException e) {
            stderr.println("missing value for " + args[args.length - 1] + "\n\n" + USAGE);
            return 2;
        } catch (NumberFormatException e) {
            stderr.println("not a number: " + e.getMessage() + "\n\n" + USAGE);
            return 2;
        }
        if (!options.projectDir.isDirectory()) {
            stderr.println("not a directory: " + options.projectDir);
            return 2;
        }
        try {
            return Lsp.run(options, stderr);
        } catch (Exception e) {
            stderr.println("snout-engine lsp failed: " + describe(e));
            return 1;
        }
    }

    static String summary(JsonObject model) {
        int synced = 0;
        int projects = 0;
        int errors = 0;
        List<String> failed = new ArrayList<>();
        for (JsonElement g : model.getAsJsonArray("gradleBuilds")) {
            JsonObject gradleBuild = g.getAsJsonObject();
            String status = gradleBuild.get("status").getAsString();
            if (!"synced".equals(status)) {
                failed.add(gradleBuild.get("id").getAsString() + " (" + gradleBuild.get("reason").getAsString()
                    + ("stale".equals(status) ? "; using the last good model" : "") + ")");
                continue;
            }
            synced++;
            for (JsonElement b : gradleBuild.getAsJsonArray("builds")) {
                JsonObject build = b.getAsJsonObject();
                projects += build.getAsJsonArray("projects").size();
                errors += build.getAsJsonArray("errors").size();
                for (JsonElement p : build.getAsJsonArray("projects")) {
                    errors += p.getAsJsonObject().getAsJsonArray("errors").size();
                }
            }
        }
        return String.format("synced %d of %d Gradle build(s), %d project(s) in %d ms; %d error(s)%s",
            synced, synced + failed.size(), projects, model.get("syncMillis").getAsLong(), errors,
            failed.isEmpty() ? "" : "; failed: " + String.join(", ", failed));
    }

    static String describe(Throwable e) {
        StringBuilder sb = new StringBuilder(String.valueOf(e.getMessage()));
        for (Throwable c = e.getCause(); c != null && c != c.getCause(); c = c.getCause()) {
            sb.append("\n  caused by: ").append(c.getMessage());
        }
        return sb.toString();
    }
}
