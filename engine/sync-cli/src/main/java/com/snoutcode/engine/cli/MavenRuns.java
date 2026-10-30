package com.snoutcode.engine.cli;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import org.w3c.dom.Document;
import org.w3c.dom.Element;
import org.w3c.dom.NodeList;

import javax.xml.parsers.DocumentBuilderFactory;
import java.io.BufferedReader;
import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.io.PrintStream;
import java.net.InetAddress;
import java.net.ServerSocket;
import java.net.Socket;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Properties;
import java.util.Set;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.function.Consumer;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import java.util.stream.Stream;

/**
 * Runs Maven goals for the app, as {@link GradleRuns} runs Gradle tasks, with the same
 * notifications: {@code snout/taskOutput}, {@code snout/taskProgress} (each plugin goal, from
 * Maven's {@code --- prefix:version:goal (execution) @ module ---} lines), {@code snout/testEvent},
 * {@code snout/buildProblems} and {@code snout/taskFinished}.
 *
 * <p>Maven is the build's wrapper or the one {@link MavenHome} downloads, on the build's JDK,
 * with the user's settings and local repository (also when the app runs with another
 * {@code HOME}).
 *
 * <p>Goals for one module of a reactor run in that module alone, as an IDE runs them: first the
 * modules it depends on are compiled ({@code -pl <those> compile}), then the goals run in the
 * module with m2e's workspace reader (the unmodified {@code org.eclipse.m2e.workspace.cli} jar
 * jdtls ships, through {@code -Dmaven.ext.class.path}) resolving the other modules to their class
 * folders. So {@code spring-boot:run} or {@code test} in {@code app} neither runs in the
 * libraries nor needs them installed.
 *
 * <p>Tests report through {@code snout-junit-listener.jar}, which goes on surefire's test
 * classpath ({@code -Dmaven.test.additionalClasspath}) and sends JUnit Platform's events to a
 * socket here; a run that sent none (JUnit 4 alone) is read from surefire's XML reports.
 */
final class MavenRuns {
    /** What to run. */
    static final class Invocation {
        ProjectSync.Plan plan;
        /** The module to run in; null for the whole build. */
        Modules.Ref module;
        /** The modules of the reactor, to resolve the module's dependencies on others. */
        List<Modules.Ref> reactor = List.of();
        final List<String> goals = new ArrayList<>();
        final List<String> args = new ArrayList<>();
        final List<String> profiles = new ArrayList<>();
        final Map<String, String> env = new LinkedHashMap<>();
        boolean offline;
        boolean skipTests;
        /** Surefire's {@code -Dtest}: {@code Class}, {@code Class#method+other}, {@code pkg/**}{@code /*}. */
        String testFilter;
        /** Report tests through the listener. */
        boolean tests;
        /** A JDWP port for the tests' JVM ({@code tests}) or {@code spring-boot:run}'s ({@code boot}). */
        int debugPort;
        String debugKind;
        Consumer<String> lineHook;
        /** Where to write the workspace state file for a module run. */
        Path runDir;
    }

    static final Pattern MOJO = Pattern.compile("--- ([\\w.-]+):([\\w.-]+):([\\w.-]+) \\(([^)]*)\\) @ ([\\w.-]+) ---");
    static final Pattern REACTOR_FAILED = Pattern.compile("^\\[ERROR] Failed to execute goal (.*)$");

    private MavenRuns() {
    }

    /** The full command lines an invocation runs, in order (the first compiles the module's dependencies). */
    static List<List<String>> commands(Invocation inv, PrintStream progress) throws IOException {
        File root = inv.plan.rootDir();
        List<List<String>> out = new ArrayList<>();
        List<String> upstream = inv.module == null ? List.of() : upstream(inv.module, inv.reactor);
        boolean single = inv.module != null && !":".equals(inv.module.gradlePath());
        File m2e = m2eWorkspaceJar();
        boolean workspace = single && !upstream.isEmpty() && m2e != null;
        if (workspace) {
            List<String> first = base(inv, progress);
            first.add("-pl");
            first.add(String.join(",", upstream));
            first.add(upstreamTests(inv.module) ? "test-compile" : "compile");
            out.add(first);
        }
        List<String> run = base(inv, progress);
        if (single) {
            run.add("-pl");
            run.add(inv.module.gradlePath());
            if (!upstream.isEmpty() && !workspace) {
                run.add("-am");
            }
        }
        if (workspace) {
            Path state = workspaceState(inv);
            run.add("-Dmaven.ext.class.path=" + m2e.getAbsolutePath());
            run.add("-Dm2e.workspace.state=" + state);
        }
        if (inv.skipTests) {
            run.add("-DskipTests");
        }
        if (inv.tests) {
            File listener = listenerJar();
            if (listener != null) {
                run.add("-Dmaven.test.additionalClasspath=" + listener.getAbsolutePath());
            }
            run.add("-DtrimStackTrace=false");
        }
        if (inv.testFilter != null) {
            run.add("-Dtest=" + inv.testFilter);
            run.add("-Dsurefire.failIfNoSpecifiedTests=false");
            run.add("-DfailIfNoTests=false");
        }
        if (inv.debugPort > 0) {
            String agent = "-agentlib:jdwp=transport=dt_socket,server=y,suspend=y,address=127.0.0.1:" + inv.debugPort;
            if ("boot".equals(inv.debugKind)) {
                run.add("-Dspring-boot.run.jvmArguments=" + agent);
            } else {
                run.add("-Dmaven.surefire.debug=" + agent);
                run.add("-DforkCount=1");
            }
        }
        run.addAll(inv.goals);
        run.addAll(inv.args);
        out.add(run);
        return out;
    }

    private static List<String> base(Invocation inv, PrintStream progress) throws IOException {
        File root = inv.plan.rootDir();
        MavenEnv env = MavenEnv.load(inv.offline || inv.plan.options().offline);
        List<String> c = new ArrayList<>(MavenHome.command(root, progress));
        c.add("-B");
        if (atLeast(MavenHome.wrapperVersion(root), 3, 6)) {
            c.add("-ntp");
        }
        c.add("-f");
        c.add(new File(root, "pom.xml").getAbsolutePath());
        c.add("-Dstyle.color=never");
        c.add("-Dmaven.repo.local=" + env.localRepository);
        File settings = env.userSettingsFile();
        if (settings != null) {
            c.add("-s");
            c.add(settings.getAbsolutePath());
        }
        if (env.offline) {
            c.add("-o");
        }
        Set<String> profiles = new LinkedHashSet<>();
        if (inv.plan.mavenOptions() != null) {
            profiles.addAll(inv.plan.mavenOptions().profiles);
        }
        profiles.addAll(inv.profiles);
        if (!profiles.isEmpty()) {
            c.add("-P");
            c.add(String.join(",", profiles));
        }
        return c;
    }

    static boolean atLeast(String version, int major, int minor) {
        Matcher m = Pattern.compile("(\\d+)\\.(\\d+)").matcher(String.valueOf(version));
        if (!m.find()) {
            return true;
        }
        int a = Integer.parseInt(m.group(1));
        int b = Integer.parseInt(m.group(2));
        return a > major || a == major && b >= minor;
    }

    /** The module paths of the reactor modules a module depends on, directly or not. */
    static List<String> upstream(Modules.Ref module, List<Modules.Ref> reactor) {
        Set<String> out = new LinkedHashSet<>();
        collect(module, reactor, out, new LinkedHashSet<>());
        out.remove(module.gradlePath());
        return new ArrayList<>(out);
    }

    private static void collect(Modules.Ref module, List<Modules.Ref> reactor, Set<String> out, Set<String> seen) {
        if (!seen.add(module.gradlePath())) {
            return;
        }
        for (JsonObject set : module.sourceSets()) {
            for (String cp : List.of("compileClasspath", "runtimeClasspath")) {
                JsonArray entries = Engine.array(set, cp);
                if (entries == null) {
                    continue;
                }
                for (JsonElement e : entries) {
                    JsonObject entry = e.getAsJsonObject();
                    if (!"project".equals(Engine.string(entry, "kind"))) {
                        continue;
                    }
                    String path = Engine.string(entry, "path");
                    for (Modules.Ref other : reactor) {
                        if (other.buildId().equals(module.buildId()) && path.equals(other.gradlePath()) && !out.contains(path)) {
                            out.add(path);
                            collect(other, reactor, out, seen);
                        }
                    }
                }
            }
        }
    }

    /** Whether the module uses another module's tests (a test jar): those must compile too. */
    private static boolean upstreamTests(Modules.Ref module) {
        for (JsonObject set : module.sourceSets()) {
            JsonArray entries = Engine.array(set, "compileClasspath");
            if (entries == null) {
                continue;
            }
            for (JsonElement e : entries) {
                JsonObject entry = e.getAsJsonObject();
                if ("project".equals(Engine.string(entry, "kind"))) {
                    for (String f : GradleRuns.strings(Engine.array(entry, "files"))) {
                        if (f.endsWith("test-classes")) {
                            return true;
                        }
                    }
                }
            }
        }
        return false;
    }

    /**
     * m2e's workspace state for a run: each module of the reactor's POM, class folder (its jar)
     * and test class folder (its test jar), as {@code group:artifact:extension:classifier:version}.
     */
    private static Path workspaceState(Invocation inv) throws IOException {
        Properties state = new Properties();
        for (Modules.Ref m : inv.reactor) {
            if (!m.buildId().equals(inv.module.buildId())) {
                continue;
            }
            JsonObject maven = Engine.object(m.project(), "maven");
            String g = Engine.string(maven, "groupId");
            String a = Engine.string(maven, "artifactId");
            String v = Engine.string(maven, "version");
            if (g == null || a == null || v == null) {
                continue;
            }
            state.setProperty(g + ":" + a + ":pom::" + v, Engine.string(m.project(), "buildFile"));
            for (JsonObject set : m.sourceSets()) {
                JsonArray classes = Engine.array(Engine.object(set, "output"), "classesDirs");
                if (classes == null || classes.isEmpty()) {
                    continue;
                }
                String dir = classes.get(0).getAsString();
                if ("main".equals(Engine.string(set, "name"))) {
                    state.setProperty(g + ":" + a + ":jar::" + v, dir);
                } else {
                    state.setProperty(g + ":" + a + ":jar:tests:" + v, dir);
                    state.setProperty(g + ":" + a + ":test-jar:tests:" + v, dir);
                }
            }
        }
        Path dir = inv.runDir != null ? inv.runDir : Files.createTempDirectory("snout-maven");
        Files.createDirectories(dir);
        Path file = dir.resolve("maven-workspace-" + ProjectSync.key(inv.plan.rootDir()) + ".properties");
        try (var out = Files.newOutputStream(file)) {
            state.store(out, "The reactor's modules, for m2e's workspace reader (written by snout-engine)");
        }
        return file;
    }

    /** m2e's workspace reader jar: {@code -Dsnout.m2eWorkspace}, else the one in the bundled jdtls. */
    static File m2eWorkspaceJar() {
        String given = System.getProperty("snout.m2eWorkspace");
        if (given != null) {
            return new File(given).isFile() ? new File(given) : null;
        }
        try {
            File[] jars = new File(Lsp.installed("jdtls"), "plugins")
                .listFiles((d, n) -> n.startsWith("org.eclipse.m2e.workspace.cli_") && n.endsWith(".jar"));
            return jars == null || jars.length == 0 ? null : jars[0];
        } catch (RuntimeException e) {
            return null;
        }
    }

    /** The test listener jar: {@code -Dsnout.junitListener}, else the distribution's. */
    static File listenerJar() {
        String given = System.getProperty("snout.junitListener");
        if (given != null) {
            return new File(given).isFile() ? new File(given) : null;
        }
        try {
            File jar = new File(Lsp.installed("maven"), "snout-junit-listener.jar");
            return jar.isFile() ? jar : null;
        } catch (RuntimeException e) {
            return null;
        }
    }

    // ---- running

    /** Runs the invocation to its end, sending the notifications; returns {@code snout/taskFinished}'s parameters. */
    static JsonObject execute(Invocation inv, GradleRuns.Run run, GradleRuns.Client client) {
        BuildProblems problems = new BuildProblems();
        long started = System.currentTimeMillis();
        AtomicInteger events = new AtomicInteger();
        String outcome = "success";
        String error = null;
        Integer exit = null;
        ServerSocket server = null;
        List<Thread> readers = new ArrayList<>();
        Progress progress = new Progress(run, client);
        try {
            PrintStream note = new PrintStream(new LineOutput(line -> GradleRuns.output(run, client, "stdout", line)), true,
                StandardCharsets.UTF_8);
            List<List<String>> commands = commands(inv, note);
            if (inv.tests) {
                server = new ServerSocket(0, 50, InetAddress.getLoopbackAddress());
                ServerSocket s = server;
                Thread accept = new Thread(() -> accept(s, run, client, events, readers), "maven-test-events");
                accept.setDaemon(true);
                accept.start();
            }
            for (List<String> command : commands) {
                if (run.cancelled) {
                    outcome = "cancelled";
                    break;
                }
                GradleRuns.output(run, client, "stdout", "> " + display(command));
                ProcessBuilder builder = new ProcessBuilder(command)
                    .directory(inv.module != null ? inv.module.projectDir().toFile() : inv.plan.rootDir());
                Map<String, String> env = builder.environment();
                if (inv.plan.options().javaHome != null) {
                    env.put("JAVA_HOME", inv.plan.options().javaHome.getAbsolutePath());
                }
                String home = MavenEnv.userHome().toString();
                if (!home.equals(System.getProperty("user.home")) && MavenEnv.optsProperty("user.home") == null) {
                    String opts = env.getOrDefault("MAVEN_OPTS", "");
                    env.put("MAVEN_OPTS", (opts + " -Duser.home=" + home).trim());
                }
                env.putAll(inv.env);
                if (server != null) {
                    env.put("SNOUT_TEST_EVENTS", "127.0.0.1:" + server.getLocalPort());
                }
                Process process = builder.start();
                run.process = process;
                process.getOutputStream().close();
                Thread out = pump(process.getInputStream(), line -> line(inv, run, client, "stdout", line, problems, progress));
                Thread err = pump(process.getErrorStream(), line -> line(inv, run, client, "stderr", line, problems, progress));
                exit = process.waitFor();
                out.join(5000);
                err.join(5000);
                progress.end(exit == 0);
                if (exit != 0) {
                    outcome = run.cancelled ? "cancelled" : "failed";
                    break;
                }
            }
        } catch (IOException e) {
            outcome = run.cancelled ? "cancelled" : "failed";
            error = SnoutEngine.describe(e);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            outcome = "cancelled";
        } finally {
            if (server != null) {
                try {
                    // Give the last events a moment to arrive.
                    for (Thread t : new ArrayList<>(readers)) {
                        t.join(2000);
                    }
                    server.close();
                } catch (IOException | InterruptedException e) {
                    // closed
                }
            }
        }
        if (inv.tests && events.get() == 0) {
            reports(inv, run, client, started);
        }
        JsonObject p = new JsonObject();
        p.addProperty("runId", run.id);
        p.add("problems", BuildProblems.toJson(new ArrayList<>(new LinkedHashSet<>(problems.finish()))));
        client.notify("snout/buildProblems", p);
        return GradleRuns.finish(run, client, outcome, error, exit);
    }

    static String display(List<String> command) {
        List<String> shown = new ArrayList<>();
        for (String c : command) {
            if (c.startsWith("-Dmaven.repo.local=") || c.startsWith("-Dmaven.ext.class.path=") || c.startsWith("-Dm2e.workspace.state=")
                || c.startsWith("-Dmaven.test.additionalClasspath=") || c.equals("-Dstyle.color=never")) {
                continue;
            }
            shown.add(c.contains(" ") ? "'" + c + "'" : c);
        }
        if (!shown.isEmpty()) {
            String first = shown.get(0);
            shown.set(0, first.endsWith("mvnw") ? "./mvnw" : "mvn");
        }
        return String.join(" ", shown);
    }

    private static void accept(ServerSocket server, GradleRuns.Run run, GradleRuns.Client client, AtomicInteger events, List<Thread> readers) {
        while (!server.isClosed()) {
            try {
                Socket socket = server.accept();
                Thread t = new Thread(() -> {
                    try (BufferedReader in = new BufferedReader(new InputStreamReader(socket.getInputStream(), StandardCharsets.UTF_8))) {
                        String line;
                        while ((line = in.readLine()) != null) {
                            events.incrementAndGet();
                            GradleRuns.testEvent(run, client, line);
                        }
                    } catch (IOException e) {
                        // the test JVM is gone
                    }
                }, "maven-test-events-reader");
                t.setDaemon(true);
                synchronized (readers) {
                    readers.add(t);
                }
                t.start();
            } catch (IOException e) {
                return;
            }
        }
    }

    private static void line(Invocation inv, GradleRuns.Run run, GradleRuns.Client client, String stream, String line,
                             BuildProblems problems, Progress progress) {
        synchronized (problems) {
            problems.accept(line);
        }
        Matcher m = MOJO.matcher(line);
        if (m.find()) {
            progress.start(m.group(5) + ":" + m.group(1) + ":" + m.group(3) + " (" + m.group(4) + ")");
        }
        if (inv.lineHook != null) {
            inv.lineHook.accept(line);
        }
        GradleRuns.output(run, client, stream, line);
    }

    /** The plugin goal running now, as task progress. */
    private static final class Progress {
        private final GradleRuns.Run run;
        private final GradleRuns.Client client;
        private String current;
        private long since;

        Progress(GradleRuns.Run run, GradleRuns.Client client) {
            this.run = run;
            this.client = client;
        }

        synchronized void start(String task) {
            finish("success");
            current = task;
            since = System.currentTimeMillis();
            JsonObject p = new JsonObject();
            p.addProperty("runId", run.id);
            p.addProperty("task", task);
            p.addProperty("event", "started");
            client.notify("snout/taskProgress", p);
        }

        synchronized void end(boolean ok) {
            finish(ok ? "success" : "failed");
        }

        private void finish(String outcome) {
            if (current == null) {
                return;
            }
            JsonObject p = new JsonObject();
            p.addProperty("runId", run.id);
            p.addProperty("task", current);
            p.addProperty("event", "finished");
            p.addProperty("outcome", outcome);
            p.addProperty("millis", System.currentTimeMillis() - since);
            client.notify("snout/taskProgress", p);
            current = null;
        }
    }

    private static Thread pump(InputStream in, Consumer<String> sink) {
        Thread t = new Thread(() -> {
            try (BufferedReader reader = new BufferedReader(new InputStreamReader(in, StandardCharsets.UTF_8))) {
                String line;
                while ((line = reader.readLine()) != null) {
                    sink.accept(line);
                }
            } catch (IOException e) {
                // closed
            }
        }, "maven-output");
        t.setDaemon(true);
        t.start();
        return t;
    }

    // ---- surefire's reports, when the listener could not report

    /** Sends the tests of the surefire reports this run wrote as test events. */
    static void reports(Invocation inv, GradleRuns.Run run, GradleRuns.Client client, long since) {
        List<Path> dirs = new ArrayList<>();
        List<Modules.Ref> modules = inv.module != null ? List.of(inv.module) : inv.reactor;
        for (Modules.Ref m : modules) {
            String buildDir = Engine.string(m.project(), "buildDir");
            if (buildDir != null) {
                dirs.add(Path.of(buildDir, "surefire-reports"));
            }
        }
        for (Path dir : dirs) {
            if (!Files.isDirectory(dir)) {
                continue;
            }
            try (Stream<Path> files = Files.list(dir)) {
                for (Path f : files.filter(p -> p.getFileName().toString().startsWith("TEST-") && p.toString().endsWith(".xml")).sorted().toList()) {
                    if (Files.getLastModifiedTime(f).toMillis() + 1000 >= since) {
                        report(f, run, client);
                    }
                }
            } catch (IOException e) {
                // no reports
            }
        }
    }

    private static void report(Path file, GradleRuns.Run run, GradleRuns.Client client) {
        try {
            DocumentBuilderFactory factory = DocumentBuilderFactory.newInstance();
            factory.setFeature("http://apache.org/xml/features/disallow-doctype-decl", true);
            Document doc = factory.newDocumentBuilder().parse(file.toFile());
            Element suite = doc.getDocumentElement();
            String suiteName = suite.getAttribute("name");
            String suiteId = "report:" + suiteName;
            JsonObject s = event("started", "suite", suiteId, null, suiteName, suiteName);
            GradleRuns.testEvent(run, client, s.toString());
            NodeList cases = suite.getElementsByTagName("testcase");
            for (int i = 0; i < cases.getLength(); i++) {
                Element c = (Element) cases.item(i);
                String cls = c.getAttribute("classname");
                String name = c.getAttribute("name");
                String id = suiteId + "/" + name + "#" + i;
                GradleRuns.testEvent(run, client, event("started", "test", id, suiteId, name, cls).toString());
                JsonObject done = event("finished", "test", id, suiteId, name, cls);
                Element failure = first(c, "failure");
                if (failure == null) {
                    failure = first(c, "error");
                }
                if (failure != null) {
                    done.addProperty("outcome", "failed");
                    done.addProperty("message", failure.getAttribute("message"));
                    done.addProperty("exception", failure.getAttribute("type"));
                    done.addProperty("stackTrace", failure.getTextContent());
                } else {
                    done.addProperty("outcome", first(c, "skipped") != null ? "skipped" : "passed");
                }
                try {
                    done.addProperty("durationMillis", (long) (Double.parseDouble(c.getAttribute("time")) * 1000));
                } catch (NumberFormatException e) {
                    done.addProperty("durationMillis", 0);
                }
                GradleRuns.testEvent(run, client, done.toString());
            }
            JsonObject end = event("finished", "suite", suiteId, null, suiteName, suiteName);
            end.addProperty("outcome", "passed");
            GradleRuns.testEvent(run, client, end.toString());
        } catch (Exception e) {
            // an unreadable report adds nothing
        }
    }

    private static Element first(Element e, String tag) {
        NodeList l = e.getElementsByTagName(tag);
        return l.getLength() == 0 ? null : (Element) l.item(0);
    }

    private static JsonObject event(String event, String kind, String id, String parent, String name, String className) {
        JsonObject e = new JsonObject();
        e.addProperty("event", event);
        e.addProperty("kind", kind);
        e.addProperty("id", id);
        e.addProperty("parent", parent);
        e.addProperty("name", name);
        e.addProperty("displayName", name);
        e.addProperty("className", className);
        e.addProperty("task", "surefire");
        return e;
    }

    /** Surefire's {@code -Dtest} for a test filter: class, method, or package. */
    static String testFilter(JsonObject filter) throws Engine.RequestError {
        String kind = filter.has("kind") ? filter.get("kind").getAsString() : "all";
        return switch (kind) {
            case "all" -> null;
            case "package" -> Engine.string(filter, "package").replace('.', '/') + "/**/*";
            case "class" -> Engine.string(filter, "class");
            case "method" -> Engine.string(filter, "class") + "#" + Engine.string(filter, "method");
            default -> throw Engine.RequestError.invalid("unknown filter kind " + kind);
        };
    }

    /** {@code -Dtest} for failed tests: {@code Class#a+b,Other#c}. */
    static String rerunFilter(List<String[]> failed) {
        Map<String, List<String>> byClass = new LinkedHashMap<>();
        for (String[] t : failed) {
            List<String> methods = byClass.computeIfAbsent(t[0], k -> new ArrayList<>());
            if (t[1] != null && !methods.contains(t[1])) {
                methods.add(t[1]);
            }
        }
        List<String> parts = new ArrayList<>();
        byClass.forEach((cls, methods) -> parts.add(methods.isEmpty() ? cls : cls + "#" + String.join("+", methods)));
        return String.join(",", parts);
    }
}
