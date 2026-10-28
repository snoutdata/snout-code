package com.snoutcode.engine.cli;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

import java.io.File;
import java.io.OutputStream;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;

/** Command line entry point of snout-engine. */
public final class SnoutEngine {
    private static final String USAGE = String.join("\n",
        "usage: snout-engine sync <projectDir> [options]",
        "",
        "Fetches the Gradle build model (configuration only, no tasks run) and writes it as JSON.",
        "",
        "options:",
        "  --out <file>                write the model here instead of stdout",
        "  --java-home <dir>           JDK for the Gradle daemon (default: this JVM)",
        "  --gradle-user-home <dir>    Gradle user home (default: ~/.gradle)",
        "  --project-cache-dir <dir>   keep Gradle's project cache out of the project directory",
        "  --gradle-version <version>  use this Gradle version instead of the build's wrapper",
        "  --no-sources                do not look up source jars",
        "  --offline                   run Gradle offline",
        "  --verbose                   show Gradle's output on stderr");

    private SnoutEngine() {
    }

    public static void main(String[] args) {
        System.exit(run(args, System.out, System.err));
    }

    static int run(String[] args, PrintStream stdout, PrintStream stderr) {
        if (args.length < 2 || !"sync".equals(args[0])) {
            stderr.println(USAGE);
            return 2;
        }
        Sync.Options options = new Sync.Options();
        options.projectDir = new File(args[1]);
        File out = null;
        boolean verbose = false;
        try {
            for (int i = 2; i < args.length; i++) {
                switch (args[i]) {
                    case "--out" -> out = new File(args[++i]);
                    case "--java-home" -> options.javaHome = new File(args[++i]);
                    case "--gradle-user-home" -> options.gradleUserHome = new File(args[++i]);
                    case "--project-cache-dir" -> options.projectCacheDir = new File(args[++i]);
                    case "--gradle-version" -> options.gradleVersion = args[++i];
                    case "--no-sources" -> options.sources = false;
                    case "--offline" -> options.offline = true;
                    case "--verbose" -> verbose = true;
                    default -> {
                        stderr.println("unknown option: " + args[i] + "\n\n" + USAGE);
                        return 2;
                    }
                }
            }
        } catch (ArrayIndexOutOfBoundsException e) {
            stderr.println("missing value for " + args[args.length - 1] + "\n\n" + USAGE);
            return 2;
        }
        if (!options.projectDir.isDirectory()) {
            stderr.println("not a directory: " + options.projectDir);
            return 2;
        }
        options.gradleOutput = verbose ? stderr : OutputStream.nullOutputStream();

        try {
            JsonObject model = Sync.run(options);
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
            return hasErrors(model) ? 1 : 0;
        } catch (Exception e) {
            stderr.println("sync failed: " + describe(e));
            return 1;
        }
    }

    private static String summary(JsonObject model) {
        int projects = 0;
        int errors = 0;
        for (JsonElement b : model.getAsJsonArray("builds")) {
            JsonObject build = b.getAsJsonObject();
            projects += build.getAsJsonArray("projects").size();
            errors += build.getAsJsonArray("errors").size();
            for (JsonElement p : build.getAsJsonArray("projects")) {
                errors += p.getAsJsonObject().getAsJsonArray("errors").size();
            }
        }
        return String.format("synced %d build(s), %d project(s) with Gradle %s in %d ms; %d error(s)",
            model.getAsJsonArray("builds").size(), projects,
            model.has("gradleVersion") && !model.get("gradleVersion").isJsonNull()
                ? model.get("gradleVersion").getAsString() : "?",
            model.get("syncMillis").getAsLong(), errors);
    }

    private static boolean hasErrors(JsonObject model) {
        for (JsonElement b : model.getAsJsonArray("builds")) {
            if (!b.getAsJsonObject().getAsJsonArray("errors").isEmpty()) {
                return true;
            }
        }
        return false;
    }

    static String describe(Throwable e) {
        StringBuilder sb = new StringBuilder(String.valueOf(e.getMessage()));
        for (Throwable c = e.getCause(); c != null && c != c.getCause(); c = c.getCause()) {
            sb.append("\n  caused by: ").append(c.getMessage());
        }
        return sb.toString();
    }
}
