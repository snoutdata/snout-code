package com.snoutcode.engine.spring;

import java.util.ArrayList;
import java.util.List;

/** {@code ${key:default}} placeholders, nested ones included, in annotation values and properties. */
final class Placeholders {
    private Placeholders() {
    }

    record Key(String key, String defaultValue, int start, int end) {
    }

    /** The keys of the placeholders in a text, outermost first; offsets are those of the key in the text. */
    static List<Key> keys(String text) {
        List<Key> out = new ArrayList<>();
        if (text == null) {
            return out;
        }
        collect(text, 0, out);
        return out;
    }

    private static void collect(String text, int from, List<Key> out) {
        int i = text.indexOf("${", from);
        while (i >= 0) {
            int end = closing(text, i + 2);
            if (end < 0) {
                return;
            }
            String inner = text.substring(i + 2, end);
            int colon = separator(inner);
            String key = (colon < 0 ? inner : inner.substring(0, colon)).trim();
            String def = colon < 0 ? null : inner.substring(colon + 1);
            if (!key.isEmpty() && !key.contains("${")) {
                out.add(new Key(key, def, i + 2, i + 2 + key.length()));
            }
            // Placeholders in the default value, or in a nested key.
            collect(text.substring(0, end), i + 2, out);
            i = text.indexOf("${", end + 1);
        }
    }

    private static int closing(String text, int from) {
        int depth = 1;
        for (int j = from; j < text.length(); j++) {
            if (text.startsWith("${", j)) {
                depth++;
                j++;
            } else if (text.charAt(j) == '}') {
                depth--;
                if (depth == 0) {
                    return j;
                }
            }
        }
        return -1;
    }

    private static int separator(String inner) {
        int depth = 0;
        for (int j = 0; j < inner.length(); j++) {
            if (inner.startsWith("${", j)) {
                depth++;
                j++;
            } else if (inner.charAt(j) == '}') {
                depth--;
            } else if (inner.charAt(j) == ':' && depth == 0) {
                return j;
            }
        }
        return -1;
    }
}
