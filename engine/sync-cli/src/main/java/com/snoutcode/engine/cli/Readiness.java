package com.snoutcode.engine.cli;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

import java.net.URI;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Collection;
import java.util.Comparator;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.function.Consumer;

/**
 * How far each module is on its way to full Java support, told to the app as
 * {@code snout/readiness} notifications, one per change:
 *
 * <ul>
 *   <li>{@code syncing}: the Gradle sync runs (only the project as a whole, {@code module: null},
 *       before the modules are known);</li>
 *   <li>{@code generatingSources}: its build's source generators run (see {@link Generators});</li>
 *   <li>{@code compilingScala}: its Scala, or the shared Scala it uses, compiles for jdtls;</li>
 *   <li>{@code importing}: jdtls imports it, or reads its new Scala classes and builds;</li>
 *   <li>{@code ready}: everything works.</li>
 * </ul>
 *
 * The project as a whole ({@code module: null}) is at the least advanced stage of its modules,
 * and its percent is theirs on average.
 *
 * <p>While a module with Scala or generated sources is not ready, jdtls's diagnostics for its
 * files are held back: they would report the Scala types not compiled yet, or the code not
 * generated yet, as missing. The latest ones go out when it is ready.
 */
final class Readiness {
    enum Stage {
        SYNCING("syncing", 0), GENERATING_SOURCES("generatingSources", 5), COMPILING_SCALA("compilingScala", 10),
        IMPORTING("importing", 80), READY("ready", 100);

        final String wire;
        final int percent;

        Stage(String wire, int percent) {
            this.wire = wire;
            this.percent = percent;
        }
    }

    private static final class Module {
        final String id;
        final boolean shared;
        Stage stage = Stage.IMPORTING;
        int percent = Stage.IMPORTING.percent;
        boolean scala;
        /** It waits for generated sources (see {@link Generators}). */
        boolean generated;
        long compileStart;
        long expectedMillis = -1;

        Module(String id, boolean shared) {
            this.id = id;
            this.shared = shared;
        }
    }

    private final Consumer<JsonObject> toApp;
    private final long start = System.nanoTime();
    private final Map<String, Module> modules = new LinkedHashMap<>();
    /** Source and resource folders, deepest first, with the module that owns each. */
    private final List<Map.Entry<Path, String>> roots = new ArrayList<>();
    private final Map<String, JsonObject> held = new LinkedHashMap<>();
    private Stage projectStage = Stage.SYNCING;
    private boolean jdtlsReady;
    private Thread ticker;

    Readiness(Consumer<JsonObject> toApp) {
        this.toApp = toApp;
    }

    /** The project syncs: nothing is known of its modules yet. */
    synchronized void syncing() {
        projectStage = Stage.SYNCING;
        sendProject();
    }

    /**
     * The modules of the model, every one importing; then those whose Scala the bridge compiles,
     * {@link #compiling}.
     */
    synchronized void model(JsonObject model) {
        modules.clear();
        roots.clear();
        for (JsonElement m : array(model, "modules")) {
            JsonObject module = m.getAsJsonObject();
            String id = string(module, "id");
            if (id == null) {
                continue;
            }
            modules.put(id, new Module(id, false));
            for (JsonElement r : array(module, "sourceRoots")) {
                JsonObject root = r.getAsJsonObject();
                if (string(root, "dir") != null && !root.has("owner") && !(root.has("shared") && root.get("shared").getAsBoolean())) {
                    roots.add(Map.entry(Path.of(string(root, "dir")).toAbsolutePath().normalize(), id));
                }
            }
        }
        for (JsonElement r : array(model, "sharedSourceRoots")) {
            JsonObject root = r.getAsJsonObject();
            String name = string(root, "name");
            if (name == null || string(root, "dir") == null) {
                continue;
            }
            modules.computeIfAbsent(name, n -> new Module(n, true));
            roots.add(Map.entry(Path.of(string(root, "dir")).toAbsolutePath().normalize(), name));
        }
        roots.sort(Comparator.comparingInt((Map.Entry<Path, String> e) -> -e.getKey().getNameCount()));
        for (Module m : modules.values()) {
            m.stage = jdtlsReady ? Stage.READY : Stage.IMPORTING;
            m.percent = m.stage.percent;
            send(m);
        }
        sendProject();
    }

    /** These modules wait for a Scala compile; {@code expectedMillis} is the last one's time, or -1. */
    synchronized void compiling(Collection<String> ids, long expectedMillis) {
        for (String id : ids) {
            Module m = module(id);
            m.scala = true;
            if (m.stage != Stage.COMPILING_SCALA) {
                m.stage = Stage.COMPILING_SCALA;
                m.percent = Stage.COMPILING_SCALA.percent;
                m.compileStart = System.nanoTime();
                m.expectedMillis = expectedMillis;
                send(m);
            }
        }
        sendProject();
        startTicker();
    }

    /** These modules wait for their build's source generators. */
    synchronized void generating(Collection<String> ids) {
        for (String id : ids) {
            Module m = module(id);
            m.generated = true;
            if (m.stage != Stage.GENERATING_SOURCES) {
                m.stage = Stage.GENERATING_SOURCES;
                m.percent = Stage.GENERATING_SOURCES.percent;
                send(m);
            }
        }
        sendProject();
    }

    /** Their Scala compiled, or their sources were generated: jdtls reads them next. */
    synchronized void compiled(Collection<String> ids) {
        for (String id : ids) {
            Module m = module(id);
            m.scala |= !m.generated;
            if (m.stage != Stage.IMPORTING) {
                m.stage = Stage.IMPORTING;
                m.percent = Stage.IMPORTING.percent;
                send(m);
            }
        }
        sendProject();
    }

    /** These modules are ready: their held diagnostics go out. */
    synchronized void ready(Collection<String> ids) {
        for (String id : ids) {
            Module m = module(id);
            if (m.stage != Stage.READY) {
                m.stage = Stage.READY;
                m.percent = Stage.READY.percent;
                send(m);
            }
        }
        flush();
        sendProject();
    }

    /** jdtls imported the projects: every module that waits for nothing else is ready. */
    synchronized void jdtlsReady() {
        jdtlsReady = true;
        List<String> ready = new ArrayList<>();
        for (Module m : modules.values()) {
            if (m.stage == Stage.IMPORTING && !m.scala && !m.generated) {
                ready.add(m.id);
            }
        }
        ready(ready);
    }

    synchronized boolean isJdtlsReady() {
        return jdtlsReady;
    }

    /** Whether every module is ready. */
    synchronized boolean allReady() {
        return jdtlsReady && modules.values().stream().allMatch(m -> m.stage == Stage.READY);
    }

    /** The module whose source folder holds a file, or null. */
    synchronized String moduleOf(Path file) {
        Path f = file.toAbsolutePath().normalize();
        for (Map.Entry<Path, String> e : roots) {
            if (f.startsWith(e.getKey())) {
                return e.getKey().equals(f) ? null : e.getValue();
            }
        }
        return null;
    }

    /**
     * jdtls's {@code publishDiagnostics}: null when held back (the file's module has Scala that is
     * not in jdtls yet), else the message to send.
     */
    synchronized JsonObject diagnostics(JsonObject message) {
        JsonObject params = message.getAsJsonObject("params");
        String uri = string(params, "uri");
        Path file = path(uri);
        String id = file == null ? null : moduleOf(file);
        Module m = id == null ? null : modules.get(id);
        if (m != null && (m.scala || m.generated) && m.stage != Stage.READY) {
            held.put(uri, message);
            return null;
        }
        held.remove(uri);
        return message;
    }

    private void flush() {
        for (var it = held.entrySet().iterator(); it.hasNext(); ) {
            var e = it.next();
            Path file = path(e.getKey());
            String id = file == null ? null : moduleOf(file);
            Module m = id == null ? null : modules.get(id);
            if (m == null || m.stage == Stage.READY) {
                toApp.accept(e.getValue());
                it.remove();
            }
        }
    }

    private Module module(String id) {
        return modules.computeIfAbsent(id, i -> new Module(i, false));
    }

    /** While modules compile, their percent moves with the time the last compile took. */
    private void startTicker() {
        if (ticker != null) {
            return;
        }
        ticker = new Thread(() -> {
            try {
                while (true) {
                    Thread.sleep(1000);
                    synchronized (this) {
                        boolean any = false;
                        for (Module m : modules.values()) {
                            if (m.stage != Stage.COMPILING_SCALA) {
                                continue;
                            }
                            any = true;
                            if (m.expectedMillis > 0) {
                                long elapsed = (System.nanoTime() - m.compileStart) / 1_000_000;
                                int p = Stage.COMPILING_SCALA.percent
                                    + (int) Math.min(65, 65 * elapsed / m.expectedMillis);
                                if (p != m.percent) {
                                    m.percent = p;
                                    send(m);
                                }
                            }
                        }
                        if (!any) {
                            ticker = null;
                            sendProject();
                            return;
                        }
                        sendProject();
                    }
                }
            } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
            }
        }, "snout-readiness");
        ticker.setDaemon(true);
        ticker.start();
    }

    private void send(Module m) {
        JsonObject p = new JsonObject();
        p.addProperty("module", m.id);
        p.addProperty("stage", m.stage.wire);
        p.addProperty("percent", m.percent);
        if (m.shared) {
            p.addProperty("shared", true);
        }
        p.addProperty("millis", (System.nanoTime() - start) / 1_000_000);
        toApp.accept(Rpc.notification("snout/readiness", p));
    }

    private int lastProjectPercent = -1;
    private String lastProjectStage;

    private void sendProject() {
        Stage stage = projectStage;
        int percent = 0;
        int ready = 0;
        if (!modules.isEmpty()) {
            stage = Stage.READY;
            int sum = 0;
            for (Module m : modules.values()) {
                if (m.stage.ordinal() < stage.ordinal()) {
                    stage = m.stage;
                }
                sum += m.percent;
                ready += m.stage == Stage.READY ? 1 : 0;
            }
            percent = sum / modules.size();
        } else if (jdtlsReady) {
            stage = Stage.READY;
            percent = 100;
        }
        if (stage.wire.equals(lastProjectStage) && percent == lastProjectPercent) {
            return;
        }
        lastProjectStage = stage.wire;
        lastProjectPercent = percent;
        JsonObject p = new JsonObject();
        p.add("module", com.google.gson.JsonNull.INSTANCE);
        p.addProperty("stage", stage.wire);
        p.addProperty("percent", percent);
        p.addProperty("modules", modules.size());
        p.addProperty("ready", ready);
        p.addProperty("millis", (System.nanoTime() - start) / 1_000_000);
        toApp.accept(Rpc.notification("snout/readiness", p));
    }

    /** The current state, for {@code snout/readiness} as a request. */
    synchronized JsonObject snapshot() {
        JsonObject out = new JsonObject();
        JsonArray list = new JsonArray();
        for (Module m : modules.values()) {
            JsonObject o = new JsonObject();
            o.addProperty("module", m.id);
            o.addProperty("stage", m.stage.wire);
            o.addProperty("percent", m.percent);
            if (m.shared) {
                o.addProperty("shared", true);
            }
            list.add(o);
        }
        out.addProperty("stage", lastProjectStage == null ? projectStage.wire : lastProjectStage);
        out.addProperty("percent", Math.max(0, lastProjectPercent));
        out.add("modules", list);
        return out;
    }

    static Path path(String uri) {
        if (uri == null) {
            return null;
        }
        try {
            return uri.startsWith("file:") ? Path.of(URI.create(uri)) : Path.of(uri);
        } catch (RuntimeException e) {
            return null;
        }
    }

    /** The modules owning any of these files. */
    synchronized Set<String> modulesOf(Collection<Path> files) {
        Set<String> out = new LinkedHashSet<>();
        for (Path f : files) {
            String m = moduleOf(f);
            if (m != null) {
                out.add(m);
            }
        }
        return out;
    }

    private static String string(JsonObject o, String key) {
        return o == null || !o.has(key) || o.get(key).isJsonNull() ? null : o.get(key).getAsString();
    }

    private static JsonArray array(JsonObject o, String key) {
        return o == null || !o.has(key) || !o.get(key).isJsonArray() ? new JsonArray() : o.getAsJsonArray(key);
    }
}
