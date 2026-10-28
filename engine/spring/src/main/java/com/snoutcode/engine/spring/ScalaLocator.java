package com.snoutcode.engine.spring;

import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * Finds declarations in a Scala file by their text, for classes the Scala bridge compiled: a
 * class file says which file a class came from, not where in it its members are.
 */
final class ScalaLocator {
    final String uri;
    final String text;
    final LineIndex lines;

    ScalaLocator(String uri, String text) {
        this.uri = uri;
        this.text = text;
        this.lines = new LineIndex(text);
    }

    /** The name of {@code class|object|trait Name}, or the start of the file. */
    Loc type(String simpleName) {
        int at = typeOffset(simpleName);
        return at < 0 ? Loc.of(uri, lines, 0, 0) : Loc.of(uri, lines, at, simpleName.length());
    }

    int typeOffset(String simpleName) {
        Matcher m = Pattern.compile("\\b(?:class|object|trait)\\s+(" + Pattern.quote(simpleName) + ")\\b").matcher(text);
        return m.find() ? m.start(1) : -1;
    }

    /** A {@code val}, {@code var} or {@code def} of the type, else a constructor parameter of that name. */
    Loc member(String typeName, String name, boolean method) {
        int from = Math.max(0, typeOffset(typeName));
        String kinds = method ? "def" : "(?:va[lr]|def)";
        Matcher m = Pattern.compile("\\b" + kinds + "\\s+(" + Pattern.quote(name) + ")\\b").matcher(text);
        if (m.find(from)) {
            return Loc.of(uri, lines, m.start(1), name.length());
        }
        Loc p = parameter(typeName, name);
        return p != null ? p : type(typeName);
    }

    /** A constructor parameter {@code name:} after the class declaration. */
    Loc parameter(String typeName, String name) {
        int from = typeOffset(typeName);
        if (from < 0 || name == null) {
            return from < 0 ? null : type(typeName);
        }
        Matcher m = Pattern.compile("\\b(" + Pattern.quote(name) + ")\\s*:").matcher(text);
        if (m.find(from)) {
            return Loc.of(uri, lines, m.start(1), name.length());
        }
        return type(typeName);
    }
}
