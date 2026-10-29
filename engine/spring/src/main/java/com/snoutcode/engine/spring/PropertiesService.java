package com.snoutcode.engine.spring;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.snoutcode.engine.spring.ConfigMetadata.Hint;
import com.snoutcode.engine.spring.ConfigMetadata.Property;
import com.snoutcode.engine.spring.Facts.KeyUse;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
import java.util.function.Function;
import java.util.regex.Pattern;

/**
 * Completion, hover, validation and definition targets in an application properties or YAML
 * file, from the configuration metadata of the classpath and the keys the code reads.
 */
final class PropertiesService {
    private static final Pattern INTEGER = Pattern.compile("[-+]?\\d+");
    private static final Pattern DECIMAL = Pattern.compile("[-+]?(\\d+\\.?\\d*|\\.\\d+)([eE][-+]?\\d+)?");
    private static final Pattern DURATION = Pattern.compile("(?i)[-+]?\\d+\\s*(ns|us|ms|s|m|h|d)?|[-+]?P.*");
    private static final Pattern DATA_SIZE = Pattern.compile("(?i)\\d+\\s*(B|KB|MB|GB|TB)?");
    private static final Pattern INDEX = Pattern.compile("\\[[^\\]]*\\]");

    final ConfigMetadata meta;
    /** Relaxed key to where the code reads it. */
    final Map<String, List<KeyUse>> used;
    /** A type's enum constants, or null when it is not an enum. */
    private final Function<String, List<String>> enums;

    /** The profiles the module knows (name to where), for completion. */
    Map<String, java.util.Set<String>> knownProfiles = Map.of();
    /** The profiles active now; null while they are not evaluated. */
    java.util.Set<String> activeProfiles;
    /** The profile of the file's name ({@code application-dev.yml}), or null. */
    String fileProfile;

    PropertiesService(ConfigMetadata meta, Map<String, List<KeyUse>> used, Function<String, List<String>> enums) {
        this.meta = meta;
        this.used = used;
        this.enums = enums;
    }

    /** A match for a key that names profiles, known or not to the metadata. */
    private Match profileMatch(String key, Match m) {
        if (m != null || !Profiles.namesProfiles(key)) {
            return m;
        }
        Match p = new Match();
        p.kind = "exact";
        p.valueType = "java.lang.String";
        return p;
    }

    /** How a key in a file matches what is known. */
    static final class Match {
        Property property;
        /** exact, mapEntry, listElement, nested (inside a list or map of objects), used. */
        String kind;
        /** The type of the value at the key. */
        String valueType;
        List<KeyUse> uses = List.of();
    }

    Match find(String key) {
        String r = Names.relaxed(key);
        Match m = new Match();
        m.uses = used.getOrDefault(r, List.of());
        Property p = meta.properties.get(r);
        if (p != null) {
            m.property = p;
            m.kind = "exact";
            m.valueType = valueType(p);
            return m;
        }
        String stripped = INDEX.matcher(r).replaceAll("");
        if (!stripped.equals(r)) {
            p = meta.properties.get(stripped);
            if (p != null && isCollection(p.type) && r.endsWith("]")) {
                m.property = p;
                m.kind = "listElement";
                m.valueType = typeArgument(p.type, 0);
                return m;
            }
        }
        // A key under a Map property, or inside an element of a list of objects.
        for (int i = r.length() - 1; i > 0; i--) {
            char c = r.charAt(i);
            if (c != '.' && c != '[') {
                continue;
            }
            String prefix = r.substring(0, i);
            String base = INDEX.matcher(prefix).replaceAll("");
            Property q = meta.properties.get(base);
            if (q == null) {
                continue;
            }
            if (isMap(q.type)) {
                m.property = q;
                m.kind = "mapEntry";
                m.valueType = typeArgument(q.type, 1);
                if (m.valueType != null && !isScalar(m.valueType) && enums.apply(m.valueType) == null) {
                    m.valueType = null; // an object in the map: its own properties are not checked
                }
                return m;
            }
            if (isCollection(q.type) || (q.type != null && !isScalar(q.type))) {
                m.property = q;
                m.kind = "nested";
                return m;
            }
        }
        if (!m.uses.isEmpty()) {
            m.kind = "used";
            return m;
        }
        return null;
    }

    private String valueType(Property p) {
        Hint h = meta.hints.get(Names.relaxed(p.name));
        if (h != null && h.handleAs != null) {
            return h.handleAs;
        }
        return p.type;
    }

    /** Whether the key is a known key, a parent of one, or read by the code. */
    boolean known(String key) {
        if (find(key) != null) {
            return true;
        }
        String r = Names.relaxed(key) + ".";
        for (String k : used.keySet()) {
            if (k.startsWith(r)) {
                return true;
            }
        }
        return meta.groups.containsKey(Names.relaxed(key));
    }

    // ---- validation

    JsonArray validate(PropsDoc doc) {
        JsonArray out = new JsonArray();
        inactiveDocuments(doc, out);
        for (PropsDoc.Entry e : doc.entries) {
            if (e.key.isEmpty()) {
                continue;
            }
            if (doc.yaml && e.value == null) {
                continue; // a YAML parent key: its children are checked
            }
            Match m = find(e.key);
            if (m == null) {
                if (!known(e.key) && !childOfKnown(e.key)) {
                    out.add(diagnostic(e.line, e.keyStart, e.keyEnd, "spring.unknownProperty",
                        "Unknown property '" + e.key + "'"));
                }
                continue;
            }
            if (m.property != null && m.property.deprecated && "exact".equals(m.kind)) {
                String text = "'" + m.property.name + "' is deprecated"
                    + (m.property.replacement != null ? "; use '" + m.property.replacement + "'" : "")
                    + (m.property.reason != null ? ". " + m.property.reason : "");
                JsonObject d = diagnostic(e.line, e.keyStart, e.keyEnd, "spring.deprecatedProperty", text);
                d.add("tags", tag(2));
                if ("error".equals(m.property.deprecationLevel)) {
                    d.addProperty("message", "'" + m.property.name + "' is no longer supported"
                        + (m.property.replacement != null ? "; use '" + m.property.replacement + "'" : ""));
                }
                out.add(d);
            }
            if (e.value == null || e.complex || e.value.isEmpty() || e.value.contains("${") || m.valueType == null) {
                continue;
            }
            List<String> values = new ArrayList<>();
            String type = m.valueType;
            if ("exact".equals(m.kind) && (isCollection(type) || type.endsWith("[]"))) {
                type = type.endsWith("[]") ? type.substring(0, type.length() - 2) : typeArgument(type, 0);
                for (String v : e.value.split(",")) {
                    values.add(v.strip());
                }
            } else {
                values.add(e.value);
            }
            if (type == null) {
                continue;
            }
            for (String v : values) {
                String problem = checkValue(v, type, doc.yaml);
                if (problem != null) {
                    out.add(diagnostic(e.valueLine, e.valueStart, Math.max(e.valueEnd, e.valueStart + 1), "spring.invalidValue", problem));
                    break;
                }
            }
        }
        return out;
    }

    /** A key below a property whose type the metadata does not break down (a POJO, a list of them). */
    private boolean childOfKnown(String key) {
        String r = Names.relaxed(key);
        for (int i = r.indexOf('.'); i > 0; i = r.indexOf('.', i + 1)) {
            String prefix = INDEX.matcher(r.substring(0, i)).replaceAll("");
            Property p = meta.properties.get(prefix);
            if (p != null && (p.type == null || !isScalar(p.type))) {
                return true;
            }
        }
        return false;
    }

    /** Why a value does not fit a type; null when it does or the type is not checked. */
    String checkValue(String value, String type, boolean yaml) {
        String t = boxed(type);
        String v = value.strip();
        switch (t) {
            case "java.lang.Boolean":
                if (v.equalsIgnoreCase("true") || v.equalsIgnoreCase("false")
                    || (yaml && List.of("yes", "no", "on", "off").contains(v.toLowerCase(Locale.ROOT)))) {
                    return null;
                }
                return "'" + v + "' is not a boolean: use true or false";
            case "java.lang.Integer", "java.lang.Long", "java.lang.Short", "java.lang.Byte", "java.math.BigInteger":
                return INTEGER.matcher(v).matches() ? null : "'" + v + "' is not a whole number (" + Names.simple(t) + ")";
            case "java.lang.Double", "java.lang.Float", "java.math.BigDecimal":
                return DECIMAL.matcher(v).matches() ? null : "'" + v + "' is not a number (" + Names.simple(t) + ")";
            case "java.time.Duration":
                return DURATION.matcher(v).matches() ? null : "'" + v + "' is not a duration: use 10s, 5m, 2h, 500ms or ISO-8601 (PT10S)";
            case "org.springframework.util.unit.DataSize":
                return DATA_SIZE.matcher(v).matches() ? null : "'" + v + "' is not a data size: use 10MB, 512KB, 1GB";
            default:
                break;
        }
        List<String> constants = enums.apply(t);
        if (constants != null && !constants.isEmpty()) {
            String rv = Names.relaxed(v);
            for (String c : constants) {
                if (Names.relaxed(c).equals(rv)) {
                    return null;
                }
            }
            return "'" + v + "' is not one of " + String.join(", ", lower(constants));
        }
        return null;
    }

    private static List<String> lower(List<String> constants) {
        List<String> out = new ArrayList<>();
        for (String c : constants) {
            out.add(c.toLowerCase(Locale.ROOT).replace('_', '-'));
        }
        return out;
    }

    /**
     * While profiles are evaluated: a hint on a document (or a whole {@code application-<profile>}
     * file) the active profiles leave out, as Spring will not read it.
     */
    private void inactiveDocuments(PropsDoc doc, JsonArray out) {
        if (activeProfiles == null) {
            return;
        }
        String active = activeProfiles.isEmpty() ? "none" : String.join(", ", activeProfiles);
        if (fileProfile != null && !activeProfiles.contains(fileProfile) && !doc.entries.isEmpty()) {
            PropsDoc.Entry first = doc.entries.get(0);
            JsonObject d = diagnostic(first.line, first.keyStart, first.keyEnd, "spring.inactiveProfile",
                "Not read now: profile '" + fileProfile + "' is not active (active: " + active + ")");
            d.addProperty("severity", 4);
            out.add(d);
        }
        String on = Names.relaxed(Profiles.ON_PROFILE);
        for (PropsDoc.Entry e : doc.entries) {
            String k = Names.relaxed(e.key);
            if ((k.equals(on) || k.equals(Profiles.LEGACY_ON_PROFILE)) && e.value != null && !e.value.isBlank()) {
                String expr = PropsDoc.unquote(e.value.strip()).replace(",", "|");
                if (!Profiles.evaluate(expr, activeProfiles)) {
                    JsonObject d = diagnostic(e.valueLine, e.valueStart, e.valueEnd, "spring.inactiveProfile",
                        "Not read now: this document is for '" + expr + "', which the active profiles (" + active + ") do not match");
                    d.addProperty("severity", 4);
                    out.add(d);
                }
            }
        }
    }

    private static JsonObject diagnostic(int line, int start, int end, String code, String message) {
        JsonObject d = new JsonObject();
        d.add("range", Loc.range(line, start, line, end));
        d.addProperty("severity", 2);
        d.addProperty("source", "spring");
        d.addProperty("code", code);
        d.addProperty("message", message);
        return d;
    }

    private static JsonArray tag(int t) {
        JsonArray a = new JsonArray();
        a.add(t);
        return a;
    }

    // ---- completion

    JsonObject complete(PropsDoc doc, int line, int character) {
        String l = doc.line(line);
        int ch = Math.min(character, l.length());
        JsonArray items = new JsonArray();
        if (doc.yaml) {
            completeYaml(doc, l, line, ch, items);
        } else {
            completeProperties(doc, l, line, ch, items);
        }
        JsonObject result = new JsonObject();
        result.addProperty("isIncomplete", false);
        result.add("items", items);
        return result;
    }

    private void completeProperties(PropsDoc doc, String l, int line, int ch, JsonArray items) {
        int start = 0;
        while (start < l.length() && Character.isWhitespace(l.charAt(start))) {
            start++;
        }
        if (start < l.length() && (l.charAt(start) == '#' || l.charAt(start) == '!')) {
            return;
        }
        int sep = separator(l, start);
        if (sep < 0 || ch <= sep) {
            int keyEnd = sep < 0 ? l.length() : sep;
            while (keyEnd > start && Character.isWhitespace(l.charAt(keyEnd - 1))) {
                keyEnd--;
            }
            String typed = l.substring(start, Math.max(start, ch));
            for (Property p : candidates(typed)) {
                items.add(keyItem(p, p.name + (sep < 0 ? "=" : ""), line, start, Math.max(ch, keyEnd)));
            }
            for (String k : usedCandidates(typed)) {
                items.add(usedItem(k, k + (sep < 0 ? "=" : ""), line, start, Math.max(ch, keyEnd)));
            }
            return;
        }
        String key = l.substring(start, sep).strip();
        int valueStart = sep + 1;
        while (valueStart < l.length() && valueStart < ch && Character.isWhitespace(l.charAt(valueStart))) {
            valueStart++;
        }
        Match m = profileMatch(key, find(key));
        if (m != null) {
            int from = valueStart;
            String type = m.valueType;
            if (type != null && isCollection(type)) {
                int comma = l.lastIndexOf(',', ch - 1);
                if (comma >= valueStart) {
                    from = comma + 1;
                }
                type = typeArgument(type, 0);
            }
            valueItems(key, m, type, line, from, l.length(), items);
        }
    }

    /** The '=' or ':' (or the whitespace) that ends a properties key. */
    static int separator(String l, int start) {
        for (int i = start; i < l.length(); i++) {
            char c = l.charAt(i);
            if (c == '\\') {
                i++;
            } else if (c == '=' || c == ':') {
                return i;
            } else if (Character.isWhitespace(c)) {
                int j = i;
                while (j < l.length() && Character.isWhitespace(l.charAt(j))) {
                    j++;
                }
                return j < l.length() && (l.charAt(j) == '=' || l.charAt(j) == ':') ? j : (j < l.length() ? i : -1);
            }
        }
        return -1;
    }

    private void completeYaml(PropsDoc doc, String l, int line, int ch, JsonArray items) {
        String content = PropsDoc.stripComment(l);
        int indent = PropsDoc.indentOf(content);
        int keyStart = indent;
        if (content.startsWith("- ", keyStart)) {
            keyStart += 2;
        }
        int colon = PropsDoc.keyColon(content, keyStart);
        if (colon >= 0 && ch > colon) {
            String parent = doc.yamlParent(line, indent);
            String key = PropsDoc.unquote(content.substring(keyStart, colon).strip());
            String full = parent.isEmpty() ? key : parent + "." + key;
            Match m = profileMatch(full, find(full));
            if (m != null) {
                int from = colon + 1;
                while (from < l.length() && from < ch && l.charAt(from) == ' ') {
                    from++;
                }
                valueItems(full, m, m.valueType, line, from, content.stripTrailing().length(), items);
            }
            return;
        }
        if (content.isBlank()) {
            indent = ch;
            keyStart = ch;
        }
        String parent = doc.yamlParent(line, indent);
        String typed = content.substring(Math.min(keyStart, content.length()), Math.max(Math.min(keyStart, content.length()), Math.min(ch, content.length())));
        String prefix = parent.isEmpty() ? typed : parent + "." + typed;
        int depth = parent.isEmpty() ? 0 : segments(parent).size();
        int end = colon >= 0 ? colon + 1 : Math.max(ch, content.stripTrailing().length());
        Set<String> seen = new LinkedHashSet<>();
        for (Property p : candidates(prefix)) {
            List<String> parts = segments(p.name);
            if (parts.size() <= depth) {
                continue;
            }
            String rest = String.join(".", parts.subList(depth, parts.size()));
            if (seen.add(rest)) {
                items.add(keyItem(p, rest + ": ", line, keyStart, end));
            }
        }
    }

    private static List<String> segments(String name) {
        List<String> out = new ArrayList<>();
        for (String s : name.split("\\.")) {
            out.add(s);
        }
        return out;
    }

    private List<Property> candidates(String typed) {
        String r = Names.relaxed(typed);
        List<Property> out = new ArrayList<>();
        for (Property p : meta.properties.values()) {
            if (Names.relaxed(p.name).startsWith(r)) {
                out.add(p);
            }
        }
        return out;
    }

    private List<String> usedCandidates(String typed) {
        String r = Names.relaxed(typed);
        Set<String> out = new LinkedHashSet<>();
        for (Map.Entry<String, List<KeyUse>> e : used.entrySet()) {
            if (e.getKey().startsWith(r) && !meta.properties.containsKey(e.getKey()) && !e.getValue().isEmpty()) {
                out.add(e.getValue().get(0).key);
            }
        }
        return new ArrayList<>(out);
    }

    private JsonObject keyItem(Property p, String insert, int line, int start, int end) {
        JsonObject item = new JsonObject();
        item.addProperty("label", p.name);
        item.addProperty("kind", 10);
        if (p.type != null) {
            item.addProperty("detail", Names.simple(p.type));
        }
        item.add("documentation", markdown(doc(p)));
        item.addProperty("filterText", insert.substring(0, insert.length() - (insert.endsWith(": ") ? 2 : insert.endsWith("=") ? 1 : 0)));
        item.addProperty("sortText", (p.deprecated ? "1" : "0") + p.name);
        if (p.deprecated) {
            item.add("tags", tag(1));
        }
        JsonObject edit = new JsonObject();
        edit.add("range", Loc.range(line, start, line, end));
        edit.addProperty("newText", insert);
        item.add("textEdit", edit);
        return item;
    }

    private JsonObject usedItem(String key, String insert, int line, int start, int end) {
        JsonObject item = new JsonObject();
        item.addProperty("label", key);
        item.addProperty("kind", 10);
        item.addProperty("detail", "read by the code");
        item.addProperty("sortText", "0" + key);
        JsonObject edit = new JsonObject();
        edit.add("range", Loc.range(line, start, line, end));
        edit.addProperty("newText", insert);
        item.add("textEdit", edit);
        return item;
    }

    private void valueItems(String key, Match m, String type, int line, int start, int end, JsonArray items) {
        Map<String, String> values = new LinkedHashMap<>();
        int kind = 12;
        if (Profiles.namesProfiles(key)) {
            for (Map.Entry<String, java.util.Set<String>> p : knownProfiles.entrySet()) {
                boolean on = activeProfiles != null && activeProfiles.contains(p.getKey());
                values.putIfAbsent(p.getKey(), (on ? "Active now. " : "") + "From " + String.join("; ", p.getValue()) + ".");
            }
        }
        if (m.property != null) {
            String r = Names.relaxed(m.property.name);
            for (String name : List.of(r, r + ".values")) {
                Hint h = meta.hints.get(name);
                if (h != null) {
                    for (String[] v : h.values) {
                        values.putIfAbsent(v[0], v[1]);
                    }
                }
            }
        }
        if (type != null) {
            String t = boxed(type);
            if ("java.lang.Boolean".equals(t)) {
                values.putIfAbsent("true", null);
                values.putIfAbsent("false", null);
            } else {
                List<String> constants = enums.apply(t);
                if (constants != null) {
                    kind = 20;
                    for (String c : lower(constants)) {
                        values.putIfAbsent(c, null);
                    }
                }
            }
        }
        for (Map.Entry<String, String> v : values.entrySet()) {
            JsonObject item = new JsonObject();
            item.addProperty("label", v.getKey());
            item.addProperty("kind", kind);
            if (v.getValue() != null) {
                item.add("documentation", markdown(v.getValue()));
            }
            JsonObject edit = new JsonObject();
            edit.add("range", Loc.range(line, start, line, Math.max(start, end)));
            edit.addProperty("newText", v.getKey());
            item.add("textEdit", edit);
            items.add(item);
        }
    }

    // ---- hover and definition

    /** The full key at a position, and the range of its text there; null when there is none. */
    PropsDoc.Entry keyAt(PropsDoc doc, int line, int character) {
        for (PropsDoc.Entry e : doc.entries) {
            if (e.line == line && character >= e.keyStart && character <= e.keyEnd) {
                return e;
            }
        }
        return null;
    }

    JsonObject hover(PropsDoc doc, int line, int character) {
        PropsDoc.Entry e = keyAt(doc, line, character);
        if (e == null) {
            return null;
        }
        Match m = find(e.key);
        String text;
        if (m == null) {
            return null;
        } else if (m.property != null) {
            text = doc(m.property);
            if (!"exact".equals(m.kind) && m.valueType != null) {
                text = "**" + e.key + "** `" + Names.simple(m.valueType) + "`\n\nAn entry of:\n\n" + text;
            }
        } else {
            StringBuilder s = new StringBuilder("**" + e.key + "**\n\nRead by the code:\n");
            for (KeyUse u : m.uses) {
                s.append("\n- `").append(u.loc == null ? "" : fileName(u.loc.uri) + ":" + (u.loc.line + 1)).append('`');
                if (u.defaultValue != null) {
                    s.append(" (default `").append(u.defaultValue).append("`)");
                }
            }
            text = s.toString();
        }
        JsonObject hover = new JsonObject();
        hover.add("contents", markdown(text));
        hover.add("range", Loc.range(e.line, e.keyStart, e.line, e.keyEnd));
        return hover;
    }

    private static String fileName(String uri) {
        return uri.substring(uri.lastIndexOf('/') + 1);
    }

    static String doc(Property p) {
        StringBuilder s = new StringBuilder();
        s.append("**").append(p.name).append("**");
        if (p.type != null) {
            s.append(" `").append(Names.simple(p.type)).append('`');
        }
        if (p.deprecated) {
            s.append("\n\n*Deprecated*");
            if (p.replacement != null) {
                s.append(": use `").append(p.replacement).append('`');
            }
            if (p.reason != null) {
                s.append(". ").append(p.reason);
            }
        }
        if (p.description != null) {
            s.append("\n\n").append(p.description);
        }
        if (p.defaultValue != null) {
            s.append("\n\nDefault: `").append(p.defaultValue).append('`');
        }
        if (p.sourceType != null) {
            s.append("\n\nDeclared by `").append(Names.display(p.sourceType)).append('`');
        }
        return s.toString();
    }

    private static JsonObject markdown(String text) {
        JsonObject o = new JsonObject();
        o.addProperty("kind", "markdown");
        o.addProperty("value", text);
        return o;
    }

    // ---- types

    static boolean isMap(String type) {
        return type != null && (type.startsWith("java.util.Map<") || type.startsWith("java.util.LinkedHashMap<")
            || type.startsWith("java.util.HashMap<") || type.startsWith("java.util.TreeMap<") || type.startsWith("java.util.SortedMap<")
            || type.equals("java.util.Map") || type.equals("java.util.Properties"));
    }

    static boolean isCollection(String type) {
        return type != null && (type.startsWith("java.util.List") || type.startsWith("java.util.Set") || type.startsWith("java.util.Collection")
            || type.startsWith("java.util.SortedSet") || type.startsWith("java.util.LinkedHashSet") || type.startsWith("java.util.ArrayList")
            || type.endsWith("[]"));
    }

    static boolean isScalar(String type) {
        String t = boxed(type);
        return t.startsWith("java.lang.") || t.startsWith("java.time.") || t.startsWith("java.math.")
            || t.equals("org.springframework.util.unit.DataSize") || t.equals("java.nio.charset.Charset")
            || t.equals("java.util.Locale") || t.equals("java.net.URI") || t.equals("java.net.URL")
            || t.equals("java.net.InetAddress") || t.equals("java.io.File") || t.equals("java.nio.file.Path")
            || t.equals("org.springframework.core.io.Resource") || t.equals("java.util.regex.Pattern")
            || t.equals("java.util.TimeZone") || t.equals("java.util.UUID") || t.equals("org.springframework.util.MimeType")
            || t.equals("org.springframework.http.MediaType") || isCollection(t) || isMap(t);
    }

    static String boxed(String type) {
        if (type == null) {
            return "";
        }
        return switch (type) {
            case "boolean" -> "java.lang.Boolean";
            case "int" -> "java.lang.Integer";
            case "long" -> "java.lang.Long";
            case "short" -> "java.lang.Short";
            case "byte" -> "java.lang.Byte";
            case "double" -> "java.lang.Double";
            case "float" -> "java.lang.Float";
            case "char" -> "java.lang.Character";
            default -> type;
        };
    }

    /** The n-th type argument of a generic type name ({@code java.util.Map<java.lang.String,java.lang.Integer>}). */
    static String typeArgument(String type, int n) {
        if (type == null) {
            return null;
        }
        if (type.endsWith("[]")) {
            return n == 0 ? type.substring(0, type.length() - 2) : null;
        }
        int open = type.indexOf('<');
        if (open < 0 || !type.endsWith(">")) {
            return null;
        }
        String inner = type.substring(open + 1, type.length() - 1);
        List<String> args = new ArrayList<>();
        int depth = 0;
        int from = 0;
        for (int i = 0; i < inner.length(); i++) {
            char c = inner.charAt(i);
            if (c == '<') {
                depth++;
            } else if (c == '>') {
                depth--;
            } else if (c == ',' && depth == 0) {
                args.add(inner.substring(from, i).strip());
                from = i + 1;
            }
        }
        args.add(inner.substring(from).strip());
        return n < args.size() ? args.get(n) : null;
    }
}
