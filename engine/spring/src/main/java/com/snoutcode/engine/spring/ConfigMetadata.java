package com.snoutcode.engine.spring;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Spring Boot's configuration metadata ({@code META-INF/spring-configuration-metadata.json} and
 * {@code additional-spring-configuration-metadata.json}), merged from every jar of a classpath
 * and the {@code @ConfigurationProperties} classes of the sources. Keys are kept in relaxed form.
 */
final class ConfigMetadata {
    static final class Property {
        String name;
        String type;
        String description;
        String defaultValue;
        String sourceType;
        String sourceMethod;
        boolean deprecated;
        String deprecationLevel;
        String replacement;
        String reason;
        /** Where it is declared in the sources; null for a jar's metadata. */
        Loc loc;
        /** The jar or file the metadata came from, or {@code source}. */
        String origin;
    }

    static final class Hint {
        String name;
        final List<String[]> values = new ArrayList<>();
        /** A {@code handle-as} provider's target type. */
        String handleAs;
        final List<String> providers = new ArrayList<>();
    }

    final Map<String, Property> properties = new LinkedHashMap<>();
    final Map<String, Property> groups = new LinkedHashMap<>();
    final Map<String, Hint> hints = new LinkedHashMap<>();

    void read(JsonObject json, String origin) {
        for (JsonElement e : array(json, "groups")) {
            JsonObject g = e.getAsJsonObject();
            Property p = property(g, origin);
            if (p.name != null) {
                groups.putIfAbsent(Names.relaxed(p.name), p);
            }
        }
        for (JsonElement e : array(json, "properties")) {
            Property p = property(e.getAsJsonObject(), origin);
            if (p.name == null) {
                continue;
            }
            Property existing = properties.get(Names.relaxed(p.name));
            if (existing == null) {
                properties.put(Names.relaxed(p.name), p);
            } else {
                // additional metadata completes the generated one
                if (existing.description == null) {
                    existing.description = p.description;
                }
                if (existing.defaultValue == null) {
                    existing.defaultValue = p.defaultValue;
                }
                if (existing.type == null) {
                    existing.type = p.type;
                }
                if (p.deprecated) {
                    existing.deprecated = true;
                    existing.replacement = p.replacement;
                    existing.reason = p.reason;
                    existing.deprecationLevel = p.deprecationLevel;
                }
            }
        }
        for (JsonElement e : array(json, "hints")) {
            JsonObject h = e.getAsJsonObject();
            String name = string(h, "name");
            if (name == null) {
                continue;
            }
            Hint hint = hints.computeIfAbsent(Names.relaxed(name), k -> new Hint());
            hint.name = name;
            for (JsonElement v : array(h, "values")) {
                JsonObject value = v.getAsJsonObject();
                JsonElement val = value.get("value");
                if (val != null && !val.isJsonNull()) {
                    hint.values.add(new String[] {val.isJsonPrimitive() ? val.getAsString() : val.toString(), string(value, "description")});
                }
            }
            for (JsonElement v : array(h, "providers")) {
                JsonObject provider = v.getAsJsonObject();
                String pname = string(provider, "name");
                hint.providers.add(pname);
                if ("handle-as".equals(pname) && provider.has("parameters")) {
                    hint.handleAs = string(provider.getAsJsonObject("parameters"), "target");
                }
            }
        }
    }

    void add(Property p) {
        properties.putIfAbsent(Names.relaxed(p.name), p);
    }

    private static Property property(JsonObject o, String origin) {
        Property p = new Property();
        p.name = string(o, "name");
        p.type = string(o, "type");
        p.description = string(o, "description");
        p.sourceType = string(o, "sourceType");
        p.sourceMethod = string(o, "sourceMethod");
        p.origin = origin;
        JsonElement def = o.get("defaultValue");
        if (def != null && !def.isJsonNull()) {
            if (def.isJsonArray()) {
                List<String> parts = new ArrayList<>();
                def.getAsJsonArray().forEach(x -> parts.add(x.isJsonPrimitive() ? x.getAsString() : x.toString()));
                p.defaultValue = String.join(",", parts);
            } else {
                p.defaultValue = def.isJsonPrimitive() ? def.getAsString() : def.toString();
            }
        }
        if (o.has("deprecation") && o.get("deprecation").isJsonObject()) {
            JsonObject d = o.getAsJsonObject("deprecation");
            p.deprecated = true;
            p.deprecationLevel = string(d, "level");
            p.replacement = string(d, "replacement");
            p.reason = string(d, "reason");
        } else if (o.has("deprecated") && o.get("deprecated").getAsBoolean()) {
            p.deprecated = true;
        }
        return p;
    }

    static String string(JsonObject o, String key) {
        JsonElement e = o == null ? null : o.get(key);
        return e == null || e.isJsonNull() ? null : e.getAsString();
    }

    static JsonArray array(JsonObject o, String key) {
        JsonElement e = o == null ? null : o.get(key);
        return e != null && e.isJsonArray() ? e.getAsJsonArray() : new JsonArray();
    }
}
