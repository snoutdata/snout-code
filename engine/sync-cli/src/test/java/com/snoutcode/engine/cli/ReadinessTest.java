package com.snoutcode.engine.cli;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.Test;

import java.nio.file.Path;
import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertTrue;

/** The readiness the engine reports per module, and the diagnostics it holds back meanwhile. */
class ReadinessTest {
    private final List<JsonObject> sent = new CopyOnWriteArrayList<>();

    private static JsonObject model() {
        return JsonParser.parseString("""
            { "modules": [
                { "id": "web", "build": "web", "sourceRoots": [ { "dir": "/p/web/src/main/java", "type": "main" } ] },
                { "id": "api", "build": "api", "sourceRoots": [ { "dir": "/p/api/src/main/java", "type": "main" } ] } ],
              "sharedSourceRoots": [ { "dir": "/p/core/src/main/java", "type": "main", "name": "core", "modules": ["web"] } ] }
            """).getAsJsonObject();
    }

    private static JsonObject diagnostics(String file) {
        JsonObject params = new JsonObject();
        params.addProperty("uri", Path.of(file).toUri().toString());
        params.add("diagnostics", new JsonArray());
        return Rpc.notification("textDocument/publishDiagnostics", params);
    }

    private JsonObject last(String module) {
        JsonObject out = null;
        for (JsonObject m : sent) {
            if ("snout/readiness".equals(m.get("method").getAsString())) {
                JsonObject p = m.getAsJsonObject("params");
                if (module == null ? p.get("module").isJsonNull() : !p.get("module").isJsonNull() && module.equals(p.get("module").getAsString())) {
                    out = p;
                }
            }
        }
        return out;
    }

    @Test
    void reportsEachModuleAndHoldsTheDiagnosticsOfModulesWaitingForScala() {
        Readiness r = new Readiness(sent::add);
        r.syncing();
        assertEquals("syncing", last(null).get("stage").getAsString());
        r.model(model());
        r.compiling(List.of("core", "web"), 10_000);
        assertEquals("compilingScala", last("web").get("stage").getAsString());
        assertTrue(last("core").get("shared").getAsBoolean());

        JsonObject web = diagnostics("/p/web/src/main/java/demo/A.java");
        JsonObject core = diagnostics("/p/core/src/main/java/demo/B.java");
        JsonObject api = diagnostics("/p/api/src/main/java/demo/C.java");
        assertNull(r.diagnostics(web), "held: its Scala is not in jdtls yet");
        assertNull(r.diagnostics(core));
        assertSame(api, r.diagnostics(api), "a module without Scala");

        r.jdtlsReady();
        assertEquals("ready", last("api").get("stage").getAsString());
        assertEquals("compilingScala", last(null).get("stage").getAsString(), "the least advanced module");

        r.compiled(List.of("core"));
        assertEquals("importing", last("core").get("stage").getAsString());
        int before = sent.size();
        r.ready(List.of("core"));
        assertTrue(sent.subList(before, sent.size()).contains(core), "core's held diagnostics go out");
        assertTrue(!sent.contains(web));

        r.compiled(List.of("web"));
        r.ready(List.of("web"));
        assertTrue(sent.contains(web));
        assertEquals("ready", last(null).get("stage").getAsString());
        assertEquals(100, last(null).get("percent").getAsInt());
        assertEquals(3, last(null).get("ready").getAsInt());
        assertTrue(r.allReady());
        assertEquals("web", r.moduleOf(Path.of("/p/web/src/main/java/demo/A.java")));
    }
}
