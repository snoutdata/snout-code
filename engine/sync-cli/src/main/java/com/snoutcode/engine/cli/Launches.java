package com.snoutcode.engine.cli;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonNull;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.net.InetAddress;
import java.net.ServerSocket;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Properties;
import java.util.Set;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;
import java.util.function.Consumer;

/**
 * Launches a main class the way the editor sees it: jdtls (through java-debug) resolves the
 * class's project, classpath and Java, from the JDT projects the importer built, so the code
 * runs as jdtls compiled it, with the Scala bridge classes and the libraries of the model. The
 * resource folders of the module and the modules it depends on go on the classpath too, as
 * jdtls does not copy resources.
 *
 * <p>In debug mode the JVM starts with a JDWP agent that waits for the debugger, and java-debug
 * opens a DAP session ({@code vscode.java.startDebugSession}); the app connects to its port and
 * attaches with the arguments it gets back.
 */
final class Launches {
    private Launches() {
    }

    /** What a launch resolves to. */
    record Resolved(String mainClass, String projectName, String java, List<String> classpath, List<String> modulepath,
                    Path workingDir, List<String> jvmArgs, List<String> args, Map<String, String> env) {
    }

    static JsonObject main(Engine engine, JsonObject config, boolean debug, boolean dryRun) throws Exception {
        Resolved r = resolve(engine, config);
        if (dryRun) {
            JsonObject out = new JsonObject();
            out.addProperty("mainClass", r.mainClass());
            out.addProperty("projectName", r.projectName());
            out.addProperty("java", r.java());
            out.add("classpath", array(r.classpath()));
            out.add("modulepath", array(r.modulepath()));
            out.addProperty("workingDir", r.workingDir().toString());
            JsonObject env = new JsonObject();
            r.env().forEach(env::addProperty);
            out.add("env", env);
            out.add("command", array(command(r, null, null)));
            return out;
        }
        GradleRuns.Run run = engine.register();
        int jdwp = debug ? freePort() : -1;
        Integer dapPort = debug ? startDebugSession(engine) : null;
        Path argFile = engine.runDir().resolve(run.id + "-" + ProcessHandle.current().pid() + ".args");
        List<String> command = command(r, jdwp > 0 ? jdwp : null, argFile);
        ProcessBuilder builder = new ProcessBuilder(command).directory(r.workingDir().toFile());
        builder.environment().putAll(r.env());
        if (argFile == null || !Files.exists(argFile)) {
            builder.environment().put("CLASSPATH", String.join(java.io.File.pathSeparator, r.classpath()));
        }
        CompletableFuture<Integer> listening = new CompletableFuture<>();
        Process process = builder.start();
        run.process = process;
        process.getOutputStream().close();
        Consumer<String> watch = line -> {
            int port = GradleRuns.jdwpPort(line);
            if (port > 0) {
                listening.complete(port);
            }
        };
        Thread stdout = pump(process.getInputStream(), run, engine, "stdout", watch);
        Thread stderr = pump(process.getErrorStream(), run, engine, "stderr", watch);
        engine.pool().submit(() -> {
            try {
                int exit = process.waitFor();
                stdout.join(5000);
                stderr.join(5000);
                listening.complete(-1);
                GradleRuns.finish(run, engine, run.cancelled ? "cancelled" : exit == 0 ? "success" : "failed", null, exit);
            } catch (InterruptedException e) {
                process.destroy();
            } finally {
                engine.unregister(run);
                if (argFile != null) {
                    try {
                        Files.deleteIfExists(argFile);
                    } catch (IOException e) {
                        // left in the cache's run folder
                    }
                }
            }
            return null;
        });
        JsonObject out = new JsonObject();
        out.addProperty("runId", run.id);
        out.addProperty("pid", process.pid());
        out.addProperty("mainClass", r.mainClass());
        out.addProperty("projectName", r.projectName());
        if (debug) {
            JsonObject attach = attachArguments(jdwp, r.projectName());
            JsonObject d = new JsonObject();
            d.addProperty("dapPort", dapPort);
            d.add("attach", attach);
            out.add("debug", d);
            int port;
            try {
                port = listening.get(30, TimeUnit.SECONDS);
            } catch (TimeoutException e) {
                port = -1;
            }
            d.addProperty("listening", port > 0);
            if (port > 0) {
                JsonObject p = new JsonObject();
                p.addProperty("runId", run.id);
                p.addProperty("dapPort", dapPort);
                p.add("attach", attach);
                engine.notify("snout/debugReady", p);
            }
        }
        return out;
    }

    /** The command line; with an argument file, the classpath goes into it (Java 9 and later). */
    static List<String> command(Resolved r, Integer jdwpPort, Path argFile) throws IOException {
        List<String> command = new ArrayList<>();
        command.add(r.java());
        if (jdwpPort != null) {
            command.add("-agentlib:jdwp=transport=dt_socket,server=y,suspend=y,address=127.0.0.1:" + jdwpPort);
        }
        command.addAll(r.jvmArgs());
        boolean modern = javaFeature(r.java()) >= 9;
        if (argFile != null && modern) {
            StringBuilder text = new StringBuilder();
            if (!r.modulepath().isEmpty()) {
                text.append("--module-path\n").append(quote(String.join(java.io.File.pathSeparator, r.modulepath()))).append('\n');
            }
            text.append("-cp\n").append(quote(String.join(java.io.File.pathSeparator, r.classpath()))).append('\n');
            Files.createDirectories(argFile.getParent());
            Files.writeString(argFile, text.toString());
            command.add("@" + argFile);
        } else if (argFile == null) {
            if (!r.modulepath().isEmpty()) {
                command.add("--module-path");
                command.add(String.join(java.io.File.pathSeparator, r.modulepath()));
            }
            command.add("-cp");
            command.add(String.join(java.io.File.pathSeparator, r.classpath()));
        }
        command.add(r.mainClass());
        command.addAll(r.args());
        return command;
    }

    /** An argument file token: quoted, with backslashes and quotes escaped. */
    private static String quote(String s) {
        return "\"" + s.replace("\\", "\\\\").replace("\"", "\\\"") + "\"";
    }

    /** Resolves the main class's project, classpath and Java through jdtls and java-debug. */
    static Resolved resolve(Engine engine, JsonObject config) throws Exception {
        String mainClass = Engine.string(config, "mainClass");
        if (mainClass == null) {
            throw Engine.RequestError.invalid("the configuration has no main class");
        }
        Modules.Ref module = Engine.string(config, "module") == null ? null : engine.module(Engine.string(config, "module"));
        String projectName = Engine.string(config, "projectName");
        JsonElement paths;
        try {
            paths = engine.executeCommand("vscode.java.resolveClasspath", args(mainClass, projectName), 600);
        } catch (Engine.RequestError e) {
            if (projectName != null) {
                throw e;
            }
            // Several projects hold the class (a shared source folder): pick the module's.
            projectName = projectFor(engine, mainClass, module);
            if (projectName == null) {
                throw e;
            }
            paths = engine.executeCommand("vscode.java.resolveClasspath", args(mainClass, projectName), 600);
        }
        JsonArray both = paths.getAsJsonArray();
        List<String> modulepath = strings(both.get(0));
        List<String> classpath = new ArrayList<>(strings(both.get(1)));
        if (projectName == null) {
            // The class's project comes first on its classpath, with its output folder in
            // jdtls's workspace: <workspace>/<project>/bin/<source set>.
            projectName = projectInWorkspace(engine.workspaceDir(), classpath);
        }
        if (projectName == null) {
            projectName = projectFor(engine, mainClass, module);
        }
        insertResources(engine, module, classpath);

        String java = null;
        try {
            JsonElement exec = engine.executeCommand("vscode.java.resolveJavaExecutable", args(mainClass, projectName), 120);
            java = exec.isJsonNull() ? null : exec.getAsString();
        } catch (Engine.RequestError e) {
            // fall back to the module's JDK
        }
        if (java == null && module != null) {
            String home = moduleJavaHome(engine.model(), module);
            if (home != null) {
                java = Path.of(home, "bin", "java").toString();
            }
        }
        if (java == null) {
            java = Path.of(System.getProperty("java.home"), "bin", "java").toString();
        }
        String dir = Engine.string(config, "workingDir");
        Path workingDir = dir != null ? Path.of(dir) : module != null ? module.projectDir() : Path.of(System.getProperty("user.dir"));
        return new Resolved(mainClass, projectName, java, classpath, modulepath, workingDir,
            GradleRuns.strings(Engine.array(config, "jvmArgs")), GradleRuns.strings(Engine.array(config, "args")),
            GradleRuns.env(config));
    }

    /** The module's JDK as the model's project view has it, else its Gradle daemon's. */
    private static String moduleJavaHome(JsonObject model, Modules.Ref module) {
        JsonArray modules = Engine.array(model, "modules");
        if (modules != null) {
            for (JsonElement m : modules) {
                JsonObject mod = m.getAsJsonObject();
                if (module.id().equals(Engine.string(mod, "id")) && mod.get("jdk") != null && mod.get("jdk").isJsonObject()) {
                    String home = Engine.string(mod.getAsJsonObject("jdk"), "javaHome");
                    if (home != null) {
                        return home;
                    }
                }
            }
        }
        return Engine.string(module.gradleBuild(), "gradleJavaHome");
    }

    /** The JDT project whose output folder is the first classpath entry inside the workspace; null if none is. */
    static String projectInWorkspace(Path workspace, List<String> classpath) {
        Path root = workspace.toAbsolutePath().normalize();
        for (String entry : classpath) {
            Path p = Path.of(entry).toAbsolutePath().normalize();
            if (p.startsWith(root) && p.getNameCount() > root.getNameCount() + 1) {
                return p.getName(root.getNameCount()).toString();
            }
        }
        return null;
    }

    /** The JDT project for a main class: java-debug's candidates, the module's by name when several. */
    private static String projectFor(Engine engine, String mainClass, Modules.Ref module) {
        try {
            JsonElement found = engine.executeCommand("vscode.java.resolveMainClass", new JsonArray(), 600);
            List<String> projects = new ArrayList<>();
            for (JsonElement e : found.getAsJsonArray()) {
                JsonObject item = e.getAsJsonObject();
                if (mainClass.equals(Engine.string(item, "mainClass")) && Engine.string(item, "projectName") != null) {
                    projects.add(Engine.string(item, "projectName"));
                }
            }
            if (module != null) {
                String name = Engine.string(module.project(), "name");
                for (String p : projects) {
                    if (p.equals(name) || p.endsWith(name)) {
                        return p;
                    }
                }
            }
            return projects.isEmpty() ? null : projects.get(0);
        } catch (Exception e) {
            return null;
        }
    }

    /**
     * Puts the resource folders of the module's main source set, and of the modules its runtime
     * classpath reaches, before the first library: jdtls compiles classes but copies no resources.
     */
    private static void insertResources(Engine engine, Modules.Ref module, List<String> classpath) {
        if (module == null) {
            return;
        }
        Set<String> dirs = new LinkedHashSet<>();
        addResources(engine, module, dirs, new LinkedHashSet<>());
        dirs.removeIf(d -> classpath.contains(d) || !Files.isDirectory(Path.of(d)));
        int at = 0;
        while (at < classpath.size() && Files.isDirectory(Path.of(classpath.get(at)))) {
            at++;
        }
        classpath.addAll(at, dirs);
    }

    private static void addResources(Engine engine, Modules.Ref module, Set<String> out, Set<String> seen) {
        if (!seen.add(module.id())) {
            return;
        }
        for (JsonObject set : module.sourceSets()) {
            if (!"main".equals(Engine.string(set, "name"))) {
                continue;
            }
            Modules.resourceDirs(set).forEach(d -> out.add(d.toString()));
            JsonArray runtime = Engine.array(set, "runtimeClasspath");
            if (runtime == null) {
                continue;
            }
            for (JsonElement e : runtime) {
                JsonObject entry = e.getAsJsonObject();
                if (!"project".equals(Engine.string(entry, "kind"))) {
                    continue;
                }
                for (Modules.Ref other : Modules.all(engine.model())) {
                    if (other.buildId().equals(module.buildId()) && String.valueOf(Engine.string(entry, "path")).equals(other.gradlePath())
                        && (Engine.string(entry, "build") == null || ":".equals(Engine.string(entry, "build"))
                            || Engine.string(entry, "build").equals(other.includedBuild())
                            || other.includedBuild() == null)) {
                        addResources(engine, other, out, seen);
                        break;
                    }
                }
            }
        }
    }

    /** Opens a java-debug DAP session in jdtls and returns the port its server listens on. */
    static int startDebugSession(Engine engine) throws Exception {
        JsonElement port = engine.executeCommand("vscode.java.startDebugSession", new JsonArray(), 600);
        if (port == null || port.isJsonNull()) {
            throw new Engine.RequestError(-32603, "java-debug did not start a debug session; is its bundle loaded?");
        }
        return port.getAsInt();
    }

    /** The DAP {@code attach} arguments for a JVM listening on a JDWP port. */
    static JsonObject attachArguments(int port, String projectName) {
        JsonObject attach = new JsonObject();
        attach.addProperty("type", "java");
        attach.addProperty("request", "attach");
        attach.addProperty("name", "SnoutCode");
        attach.addProperty("hostName", "127.0.0.1");
        attach.addProperty("port", port);
        attach.addProperty("timeout", 30000);
        if (projectName != null) {
            attach.addProperty("projectName", projectName);
        }
        return attach;
    }

    static int freePort() throws IOException {
        try (ServerSocket socket = new ServerSocket(0, 1, InetAddress.getLoopbackAddress())) {
            return socket.getLocalPort();
        }
    }

    /** The feature release of the JDK a {@code java} executable belongs to, from its {@code release} file; 0 if unknown. */
    static int javaFeature(String java) {
        Path home = Path.of(java).getParent() == null ? null : Path.of(java).getParent().getParent();
        if (home == null) {
            return 0;
        }
        Path release = home.resolve("release");
        if (!Files.isRegularFile(release)) {
            release = home.getParent() == null ? release : home.getParent().resolve("release"); // a JRE inside a JDK 8
        }
        try (InputStream in = Files.newInputStream(release)) {
            Properties p = new Properties();
            p.load(in);
            String v = p.getProperty("JAVA_VERSION", "").replace("\"", "");
            if (v.startsWith("1.")) {
                return Integer.parseInt(v.substring(2, 3));
            }
            return Integer.parseInt(v.split("[.+-]")[0]);
        } catch (IOException | RuntimeException e) {
            return 0;
        }
    }

    private static Thread pump(InputStream in, GradleRuns.Run run, Engine engine, String stream, Consumer<String> watch) {
        Thread t = new Thread(() -> {
            try (BufferedReader reader = new BufferedReader(new InputStreamReader(in, StandardCharsets.UTF_8))) {
                String line;
                while ((line = reader.readLine()) != null) {
                    watch.accept(line);
                    GradleRuns.output(run, engine, stream, line);
                }
            } catch (IOException e) {
                // the process closed its stream
            }
        }, "run-" + stream);
        t.setDaemon(true);
        t.start();
        return t;
    }

    private static JsonArray args(String mainClass, String projectName) {
        JsonArray a = new JsonArray();
        a.add(mainClass);
        a.add(projectName == null ? JsonNull.INSTANCE : new JsonPrimitive(projectName));
        return a;
    }

    private static List<String> strings(JsonElement e) {
        List<String> out = new ArrayList<>();
        if (e != null && e.isJsonArray()) {
            e.getAsJsonArray().forEach(x -> out.add(x.getAsString()));
        }
        return out;
    }

    private static JsonArray array(List<String> list) {
        JsonArray a = new JsonArray();
        list.forEach(a::add);
        return a;
    }
}
