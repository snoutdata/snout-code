package com.snoutcode.engine.spring;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.snoutcode.engine.spring.Facts.KeyUse;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

class PropertiesTest {
    private static final String METADATA = """
        { "groups": [ { "name": "server", "type": "org.example.ServerProperties" } ],
          "properties": [
            { "name": "server.port", "type": "java.lang.Integer", "description": "Server HTTP port.", "defaultValue": 8080 },
            { "name": "server.shutdown", "type": "org.example.Shutdown" },
            { "name": "server.compression.enabled", "type": "java.lang.Boolean" },
            { "name": "logging.level", "type": "java.util.Map<java.lang.String,org.example.LogLevel>" },
            { "name": "spring.profiles.active", "type": "java.util.List<java.lang.String>" },
            { "name": "spring.datasource.hikari", "type": "com.zaxxer.hikari.HikariDataSource" },
            { "name": "app.timeout", "type": "java.time.Duration" },
            { "name": "old.key", "type": "java.lang.String", "deprecation": { "replacement": "new.key", "level": "warning" } }
          ],
          "hints": [ { "name": "server.compression.enabled", "values": [ { "value": true, "description": "On." } ] } ] }""";

    private static PropertiesService service() {
        ConfigMetadata meta = new ConfigMetadata();
        meta.read(JsonParser.parseString(METADATA).getAsJsonObject(), "test.jar");
        Map<String, List<KeyUse>> used = new HashMap<>();
        KeyUse use = new KeyUse();
        use.key = "jdbc.url";
        used.computeIfAbsent(Names.relaxed(use.key), k -> new ArrayList<>()).add(use);
        return new PropertiesService(meta, used, type -> switch (type) {
            case "org.example.Shutdown" -> List.of("GRACEFUL", "IMMEDIATE");
            case "org.example.LogLevel" -> List.of("TRACE", "DEBUG", "INFO", "WARN", "ERROR", "FATAL", "OFF");
            default -> null;
        });
    }

    private static List<String> messages(JsonArray diagnostics) {
        List<String> out = new ArrayList<>();
        diagnostics.forEach(d -> out.add(d.getAsJsonObject().get("message").getAsString()));
        return out;
    }

    @Test
    void validatesKeysAndValues() {
        String text = """
            # comment
            server.port=80a
            server.shutdown=graceful
            server.shutdown = sometimes
            server.compression.enabled=yes
            logging.level.org.example=debug
            logging.level.root=loud
            spring.profiles.active=dev,prod
            spring.datasource.hikari.maximum-pool-size=5
            app.timeout=30s
            jdbc.url=jdbc:mysql://x
            nobody.reads.this=1
            old.key=x
            server.port=${PORT:8080}
            """;
        List<String> m = messages(service().validate(PropsDoc.parse(text, false)));
        assertEquals(List.of(
            "'80a' is not a whole number (Integer)",
            "'sometimes' is not one of graceful, immediate",
            "'yes' is not a boolean: use true or false",
            "'loud' is not one of trace, debug, info, warn, error, fatal, off",
            "Unknown property 'nobody.reads.this'",
            "'old.key' is deprecated; use 'new.key'"), m);
    }

    @Test
    void validatesYaml() {
        String text = """
            server:
              port: 8080
              shutdown: immediate
              compression:
                enabled: on
            logging:
              level:
                org.example: info
            spring:
              profiles:
                active:
                  - dev
                  - prod
            app:
              timeout: soon
              colour: blue
            """;
        PropsDoc doc = PropsDoc.parse(text, true);
        List<String> keys = new ArrayList<>();
        doc.entries.forEach(e -> keys.add(e.key));
        assertTrue(keys.containsAll(List.of("server.port", "server.compression.enabled", "logging.level.org.example",
            "spring.profiles.active[0]", "spring.profiles.active[1]", "app.colour")), keys.toString());
        List<String> m = messages(service().validate(doc));
        assertEquals(List.of("'soon' is not a duration: use 10s, 5m, 2h, 500ms or ISO-8601 (PT10S)", "Unknown property 'app.colour'"), m);
    }

    @Test
    void completesKeysAndValues() {
        PropertiesService s = service();
        PropsDoc doc = PropsDoc.parse("server.p\nserver.shutdown=\nserver.compression.enabled=", false);
        JsonObject keys = s.complete(doc, 0, 8);
        assertEquals(List.of("server.port"), labels(keys));
        assertEquals("server.port=", keys.getAsJsonArray("items").get(0).getAsJsonObject().getAsJsonObject("textEdit").get("newText").getAsString());
        assertEquals(List.of("graceful", "immediate"), labels(s.complete(doc, 1, 16)));
        assertEquals(List.of("true", "false"), labels(s.complete(doc, 2, 27)));
        JsonObject used = s.complete(PropsDoc.parse("jd", false), 0, 2);
        assertEquals(List.of("jdbc.url"), labels(used));
    }

    @Test
    void completesYamlRelativeToItsParent() {
        PropertiesService s = service();
        PropsDoc doc = PropsDoc.parse("server:\n  com\n  shutdown: \n", true);
        JsonObject keys = s.complete(doc, 1, 5);
        assertEquals(List.of("server.compression.enabled"), labels(keys));
        assertEquals("compression.enabled: ",
            keys.getAsJsonArray("items").get(0).getAsJsonObject().getAsJsonObject("textEdit").get("newText").getAsString());
        assertEquals(List.of("graceful", "immediate"), labels(s.complete(doc, 2, 12)));
    }

    @Test
    void hovers() {
        PropertiesService s = service();
        JsonObject hover = s.hover(PropsDoc.parse("server.port=1", false), 0, 3);
        assertNotNull(hover);
        String text = hover.getAsJsonObject("contents").get("value").getAsString();
        assertTrue(text.contains("**server.port** `Integer`") && text.contains("Server HTTP port.") && text.contains("Default: `8080`"), text);
        assertNull(s.hover(PropsDoc.parse("nothing.here=1", false), 0, 3));
    }

    @Test
    void readsPropertiesSyntax() {
        PropsDoc doc = PropsDoc.parse("a.b = one \\\n   two\nc:d\ne f\n! comment\nkey\\ with\\ space=x\n", false);
        assertEquals(4, doc.entries.size());
        assertEquals("a.b", doc.entries.get(0).key);
        assertEquals("one two", doc.entries.get(0).value);
        assertEquals("d", doc.entries.get(1).value);
        assertEquals("f", doc.entries.get(2).value);
        assertEquals("key with space", doc.entries.get(3).key);
    }

    @Test
    void offersTheKnownProfilesAndMarksTheDocumentsTheActiveOnesLeaveOut() {
        PropertiesService s = service();
        s.knownProfiles = new java.util.TreeMap<>(Map.of("dev", java.util.Set.of("application-dev.yml"),
            "prod", java.util.Set.of("@Profile on demo.Smtp")));
        s.activeProfiles = java.util.Set.of("dev");
        PropsDoc doc = PropsDoc.parse("spring.profiles.active=d\n#---\nspring.config.activate.on-profile=prod\nserver.port=1\n", false);
        assertEquals(List.of("dev", "prod"), labels(s.complete(doc, 0, 23)));
        assertTrue(s.complete(doc, 0, 23).toString().contains("Active now."), s.complete(doc, 0, 23).toString());
        List<String> hints = messages(s.validate(doc));
        assertTrue(hints.contains("Not read now: this document is for 'prod', which the active profiles (dev) do not match"),
            hints.toString());
        // A YAML group's members, and the file of a profile that is not active.
        PropsDoc yaml = PropsDoc.parse("spring:\n  profiles:\n    group:\n      dev: \n", true);
        assertEquals(List.of("dev", "prod"), labels(s.complete(yaml, 3, 11)));
        s.fileProfile = "prod";
        assertTrue(messages(s.validate(PropsDoc.parse("server.port=1\n", false)))
            .contains("Not read now: profile 'prod' is not active (active: dev)"));
        s.activeProfiles = null;
        assertTrue(messages(s.validate(doc)).stream().noneMatch(m -> m.startsWith("Not read now")));
    }

    @Test
    void readsPlaceholders() {
        List<Placeholders.Key> keys = Placeholders.keys("${a.b:${c:d}} and ${e}");
        List<String> names = new ArrayList<>();
        keys.forEach(k -> names.add(k.key()));
        assertEquals(List.of("a.b", "c", "e"), names);
        assertEquals("${c:d}", keys.get(0).defaultValue());
    }

    @Test
    void namesAsSpringDoes() {
        assertEquals("fooBar", Names.beanName("a.b.FooBar"));
        assertEquals("URLMaker", Names.beanName("a.URLMaker"));
        assertEquals("outer.Inner", Names.beanName("a.Outer$Inner"));
        assertEquals("max-pool-size", Names.kebab("maxPoolSize"));
        assertEquals(Names.relaxed("spring.datasource.maxPoolSize"), Names.relaxed("spring.datasource.max-pool-size"));
        assertEquals("/api/hello", TypeScanner.combine("/api/", "/hello"));
        assertEquals("/api", TypeScanner.combine("api", ""));
        assertEquals("/", TypeScanner.combine("", ""));
    }

    private static List<String> labels(JsonObject completion) {
        List<String> out = new ArrayList<>();
        for (JsonElement i : completion.getAsJsonArray("items")) {
            out.add(i.getAsJsonObject().get("label").getAsString());
        }
        return out;
    }
}
