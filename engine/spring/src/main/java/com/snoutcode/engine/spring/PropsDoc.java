package com.snoutcode.engine.spring;

import java.util.ArrayList;
import java.util.List;

/**
 * An {@code application*.properties} or {@code application*.yml} file as a list of entries: the
 * full key of each value (YAML's nesting flattened the way Spring flattens it, with list items
 * as {@code key[0]}), and where the key and the value are.
 */
final class PropsDoc {
    /** One key and its value. Lines and columns are 0-based; a YAML key's range is its last segment. */
    static final class Entry {
        String key;
        int line;
        int keyStart;
        int keyEnd;
        /** null for a YAML key that holds a nested block. */
        String value;
        int valueLine;
        int valueStart;
        int valueEnd;
        /** A YAML block or flow value, which is not checked against the property's type. */
        boolean complex;
        /** Which document of the file it is in, from 0: YAML's {@code ---}, or {@code #---} in a properties file. */
        int document;
    }

    final boolean yaml;
    final String text;
    final LineIndex lines;
    final List<Entry> entries = new ArrayList<>();
    /** The document the parser is in. */
    private int document;

    private PropsDoc(boolean yaml, String text) {
        this.yaml = yaml;
        this.text = text;
        this.lines = new LineIndex(text);
    }

    static boolean isYaml(String uri) {
        String u = uri.toLowerCase(java.util.Locale.ROOT);
        return u.endsWith(".yml") || u.endsWith(".yaml");
    }

    static PropsDoc parse(String text, boolean yaml) {
        PropsDoc doc = new PropsDoc(yaml, text);
        if (yaml) {
            doc.parseYaml();
        } else {
            doc.parseProperties();
        }
        return doc;
    }

    String line(int n) {
        if (n < 0 || n >= lines.lineCount()) {
            return "";
        }
        return text.substring(lines.lineStart(n), lines.lineEnd(text, n));
    }

    Entry entryAt(int line, int character) {
        for (Entry e : entries) {
            if (e.line == line && character >= e.keyStart && character <= e.keyEnd) {
                return e;
            }
            if (e.value != null && e.valueLine == line && character >= e.valueStart && character <= Math.max(e.valueEnd, e.valueStart)) {
                return e;
            }
        }
        return null;
    }

    // ---- .properties

    private void parseProperties() {
        int n = lines.lineCount();
        for (int i = 0; i < n; i++) {
            String l = line(i);
            int p = 0;
            while (p < l.length() && Character.isWhitespace(l.charAt(p))) {
                p++;
            }
            if (p >= l.length() || l.charAt(p) == '#' || l.charAt(p) == '!') {
                if (p == 0 && (l.strip().equals("#---") || l.strip().equals("!---"))) {
                    document++;
                }
                continue;
            }
            int keyStart = p;
            StringBuilder key = new StringBuilder();
            while (p < l.length()) {
                char c = l.charAt(p);
                if (c == '\\' && p + 1 < l.length()) {
                    key.append(l.charAt(p + 1));
                    p += 2;
                    continue;
                }
                if (c == '=' || c == ':' || Character.isWhitespace(c)) {
                    break;
                }
                key.append(c);
                p++;
            }
            int keyEnd = p;
            while (p < l.length() && (l.charAt(p) == ' ' || l.charAt(p) == '\t')) {
                p++;
            }
            if (p < l.length() && (l.charAt(p) == '=' || l.charAt(p) == ':')) {
                p++;
                while (p < l.length() && (l.charAt(p) == ' ' || l.charAt(p) == '\t')) {
                    p++;
                }
            }
            Entry e = new Entry();
            e.key = key.toString();
            e.line = i;
            e.keyStart = keyStart;
            e.keyEnd = keyEnd;
            e.valueLine = i;
            e.valueStart = p;
            StringBuilder value = new StringBuilder();
            String current = l;
            int q = p;
            int lineNo = i;
            while (true) {
                int end = current.length();
                boolean continued = end > q && trailingBackslashes(current) % 2 == 1;
                String part = current.substring(Math.min(q, end), continued ? end - 1 : end);
                value.append(part);
                if (!continued || lineNo + 1 >= n) {
                    break;
                }
                lineNo++;
                current = line(lineNo);
                q = 0;
                while (q < current.length() && Character.isWhitespace(current.charAt(q))) {
                    q++;
                }
                e.complex = true;
            }
            e.valueEnd = e.complex ? l.length() : l.length();
            e.value = unescape(value.toString()).strip();
            e.document = document;
            entries.add(e);
            i = lineNo;
        }
    }

    private static int trailingBackslashes(String s) {
        int count = 0;
        for (int i = s.length() - 1; i >= 0 && s.charAt(i) == '\\'; i--) {
            count++;
        }
        return count;
    }

    private static String unescape(String s) {
        if (s.indexOf('\\') < 0) {
            return s;
        }
        StringBuilder out = new StringBuilder();
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            if (c == '\\' && i + 1 < s.length()) {
                char next = s.charAt(++i);
                switch (next) {
                    case 't' -> out.append('\t');
                    case 'n' -> out.append('\n');
                    case 'r' -> out.append('\r');
                    case 'f' -> out.append('\f');
                    case 'u' -> {
                        if (i + 4 < s.length()) {
                            try {
                                out.append((char) Integer.parseInt(s.substring(i + 1, i + 5), 16));
                                i += 4;
                            } catch (NumberFormatException e) {
                                out.append('u');
                            }
                        }
                    }
                    default -> out.append(next);
                }
            } else {
                out.append(c);
            }
        }
        return out.toString();
    }

    // ---- YAML: block mappings and sequences, the subset application.yml uses

    private static final class Level {
        final int indent;
        final String path;
        int nextIndex;
        boolean holdsBlock;

        Level(int indent, String path) {
            this.indent = indent;
            this.path = path;
        }
    }

    private void parseYaml() {
        List<Level> stack = new ArrayList<>();
        int n = lines.lineCount();
        int blockIndent = -1;
        for (int i = 0; i < n; i++) {
            String raw = line(i);
            String l = stripComment(raw);
            int indent = indentOf(l);
            if (l.isBlank()) {
                continue;
            }
            if (blockIndent >= 0) {
                if (indent > blockIndent) {
                    continue; // inside a block scalar
                }
                blockIndent = -1;
            }
            String trimmed = l.strip();
            if (trimmed.equals("---") || trimmed.startsWith("--- ") || trimmed.equals("...")) {
                stack.clear();
                if (!trimmed.equals("...") && !entries.isEmpty()) {
                    document++;
                }
                continue;
            }
            boolean item = l.startsWith("-", indent) && (l.startsWith("- ", indent) || indent + 1 == l.length());
            while (!stack.isEmpty() && stack.get(stack.size() - 1).indent >= indent) {
                Level top = stack.get(stack.size() - 1);
                if (item && top.indent == indent && top.holdsBlock) {
                    break; // "key:" with its list items at the same indent
                }
                stack.remove(stack.size() - 1);
            }
            String parent = stack.isEmpty() ? "" : stack.get(stack.size() - 1).path;
            int p = indent;
            // Sequence items: "- value" or "- key: value".
            while (l.startsWith("- ", p) || (l.startsWith("-", p) && p + 1 == l.length())) {
                Level owner = stack.isEmpty() ? null : stack.get(stack.size() - 1);
                int index = owner == null ? 0 : owner.nextIndex++;
                String itemPath = parent + "[" + index + "]";
                p += 1;
                while (p < l.length() && l.charAt(p) == ' ') {
                    p++;
                }
                Level itemLevel = new Level(p - 1, itemPath);
                stack.add(itemLevel);
                parent = itemPath;
                if (p >= l.length()) {
                    break;
                }
                if (!isKeyLine(l, p)) {
                    Entry e = new Entry();
                    e.key = itemPath;
                    e.line = i;
                    e.keyStart = indent;
                    e.keyEnd = indent + 1;
                    setValue(e, l, p, i);
                    e.document = document;
                    entries.add(e);
                    if (isBlockScalar(e.value)) {
                        blockIndent = indent;
                    }
                    p = l.length();
                    break;
                }
            }
            if (p >= l.length()) {
                continue;
            }
            int colon = keyColon(l, p);
            if (colon < 0) {
                continue;
            }
            String key = unquote(l.substring(p, colon).strip());
            String full = parent.isEmpty() ? key : parent + "." + key;
            Entry e = new Entry();
            e.key = full;
            e.line = i;
            e.keyStart = p;
            e.keyEnd = p + l.substring(p, colon).stripTrailing().length();
            int v = colon + 1;
            while (v < l.length() && l.charAt(v) == ' ') {
                v++;
            }
            if (v < l.length()) {
                setValue(e, l, v, i);
                if (isBlockScalar(e.value)) {
                    blockIndent = p;
                }
            } else {
                e.valueLine = i;
                e.valueStart = l.length();
                e.valueEnd = l.length();
            }
            e.document = document;
            entries.add(e);
            Level level = new Level(p, full);
            level.holdsBlock = e.value == null;
            stack.add(level);
        }
    }

    private static void setValue(Entry e, String l, int v, int line) {
        String value = l.substring(v).strip();
        e.valueLine = line;
        e.valueStart = v;
        e.valueEnd = v + value.length();
        e.complex = value.startsWith("[") || value.startsWith("{") || isBlockScalar(value) || value.startsWith("&") || value.startsWith("*")
            || value.startsWith("!");
        e.value = unquote(value);
    }

    private static boolean isBlockScalar(String value) {
        return value != null && (value.startsWith("|") || value.startsWith(">"));
    }

    private static boolean isKeyLine(String l, int p) {
        return keyColon(l, p) >= 0;
    }

    /** The colon that ends a key at {@code p}: followed by a space or the end of the line, outside quotes. */
    static int keyColon(String l, int p) {
        char quote = 0;
        for (int i = p; i < l.length(); i++) {
            char c = l.charAt(i);
            if (quote != 0) {
                if (c == quote) {
                    quote = 0;
                }
                continue;
            }
            if ((c == '"' || c == '\'') && i == p) {
                quote = c;
            } else if (c == ':' && (i + 1 == l.length() || l.charAt(i + 1) == ' ')) {
                return i;
            } else if (c == '#' && i > p && l.charAt(i - 1) == ' ') {
                return -1;
            }
        }
        return -1;
    }

    static String unquote(String s) {
        if (s.length() >= 2 && ((s.startsWith("\"") && s.endsWith("\"")) || (s.startsWith("'") && s.endsWith("'")))) {
            return s.substring(1, s.length() - 1);
        }
        if (s.startsWith("[") && s.endsWith("]")) {
            return s.substring(1, s.length() - 1); // a flow list as Spring reads it: comma-separated
        }
        return s;
    }

    static String stripComment(String l) {
        char quote = 0;
        for (int i = 0; i < l.length(); i++) {
            char c = l.charAt(i);
            if (quote != 0) {
                if (c == quote) {
                    quote = 0;
                }
            } else if (c == '"' || c == '\'') {
                quote = c;
            } else if (c == '#' && (i == 0 || Character.isWhitespace(l.charAt(i - 1)))) {
                return l.substring(0, i);
            }
        }
        return l;
    }

    static int indentOf(String l) {
        int i = 0;
        while (i < l.length() && l.charAt(i) == ' ') {
            i++;
        }
        return i;
    }

    /**
     * The key path a YAML line at this indent nests under: the keys of the lines above with less
     * indentation. {@code ""} at the top.
     */
    String yamlParent(int line, int indent) {
        String parent = "";
        int want = indent;
        List<String> parts = new ArrayList<>();
        for (int i = line - 1; i >= 0 && want > 0; i--) {
            String l = stripComment(line(i));
            if (l.isBlank()) {
                continue;
            }
            int ind = indentOf(l);
            if (ind < want) {
                int colon = keyColon(l, ind + (l.startsWith("- ", ind) ? 2 : 0));
                if (colon > 0) {
                    String key = unquote(l.substring(ind + (l.startsWith("- ", ind) ? 2 : 0), colon).strip());
                    parts.add(0, key + (l.startsWith("- ", ind) ? "" : ""));
                }
                want = ind;
            }
        }
        for (String part : parts) {
            parent = parent.isEmpty() ? part : parent + "." + part;
        }
        return parent;
    }
}
