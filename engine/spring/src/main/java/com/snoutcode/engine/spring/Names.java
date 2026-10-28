package com.snoutcode.engine.spring;

import java.util.Locale;

/** Spring's naming rules: bean names, kebab-case property names, display names of types. */
final class Names {
    private Names() {
    }

    /** {@code a.B$C} as source writes it: {@code a.B.C}. */
    static String display(String binaryName) {
        return binaryName == null ? null : binaryName.replace('$', '.');
    }

    /** The simple name of a type, keeping its type arguments simple too. */
    static String simple(String type) {
        if (type == null) {
            return null;
        }
        StringBuilder out = new StringBuilder();
        StringBuilder word = new StringBuilder();
        for (int i = 0; i < type.length(); i++) {
            char c = type.charAt(i);
            if (Character.isJavaIdentifierPart(c) || c == '.' || c == '$') {
                word.append(c);
            } else {
                out.append(lastSegment(word.toString()));
                word.setLength(0);
                out.append(c);
            }
        }
        out.append(lastSegment(word.toString()));
        return out.toString();
    }

    private static String lastSegment(String name) {
        int i = Math.max(name.lastIndexOf('.'), name.lastIndexOf('$'));
        return i < 0 ? name : name.substring(i + 1);
    }

    /** Spring's default bean name for a component class: {@code a.FooBar} is {@code fooBar}, {@code a.URLMaker} stays. */
    static String beanName(String binaryName) {
        String shortName = binaryName.substring(binaryName.lastIndexOf('.') + 1).replace('$', '.');
        return decapitalize(shortName);
    }

    /** {@link java.beans.Introspector#decapitalize}. */
    static String decapitalize(String name) {
        if (name == null || name.isEmpty()) {
            return name;
        }
        if (name.length() > 1 && Character.isUpperCase(name.charAt(1)) && Character.isUpperCase(name.charAt(0))) {
            return name;
        }
        return Character.toLowerCase(name.charAt(0)) + name.substring(1);
    }

    static String packageOf(String binaryName) {
        int i = binaryName.lastIndexOf('.');
        return i < 0 ? "" : binaryName.substring(0, i);
    }

    /** {@code maxPoolSize} and {@code max_pool_size} as Spring writes property names: {@code max-pool-size}. */
    static String kebab(String name) {
        StringBuilder out = new StringBuilder();
        for (int i = 0; i < name.length(); i++) {
            char c = name.charAt(i);
            if (c == '_') {
                out.append('-');
            } else if (Character.isUpperCase(c)) {
                if (out.length() > 0 && out.charAt(out.length() - 1) != '-' && out.charAt(out.length() - 1) != '.') {
                    out.append('-');
                }
                out.append(Character.toLowerCase(c));
            } else {
                out.append(c);
            }
        }
        return out.toString();
    }

    /**
     * A property name in Spring's relaxed form, for comparing: each element lower case without
     * dashes or underscores, indexes kept ({@code spring.datasource.maxPoolSize} and
     * {@code spring.datasource.max-pool-size} are equal).
     */
    static String relaxed(String key) {
        StringBuilder out = new StringBuilder(key.length());
        boolean inBracket = false;
        for (int i = 0; i < key.length(); i++) {
            char c = key.charAt(i);
            if (c == '[') {
                inBracket = true;
            } else if (c == ']') {
                inBracket = false;
            }
            if (!inBracket && (c == '-' || c == '_')) {
                continue;
            }
            out.append(inBracket ? c : Character.toLowerCase(c));
        }
        return out.toString().toLowerCase(Locale.ROOT);
    }

    /** The setter or field name {@code fooBar} for the property element {@code foo-bar}. */
    static String camel(String element) {
        StringBuilder out = new StringBuilder();
        boolean upper = false;
        for (char c : element.toCharArray()) {
            if (c == '-' || c == '_') {
                upper = true;
            } else {
                out.append(upper ? Character.toUpperCase(c) : c);
                upper = false;
            }
        }
        return out.toString();
    }
}
