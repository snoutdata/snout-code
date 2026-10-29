package com.snoutcode.engine.spring;

import java.io.File;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
import java.util.TreeMap;

/**
 * Spring profiles: the expressions {@code @Profile} takes ({@code "dev"}, {@code "!prod"},
 * {@code "cloud & (eu | us)"}), and the profiles a module's application files make active:
 * {@code spring.profiles.active} (or the ones chosen in the app, as a command line would),
 * then {@code spring.profiles.include} and the groups of {@code spring.profiles.group.<name>},
 * else {@code spring.profiles.default} (or {@code default}).
 */
final class Profiles {
    private Profiles() {
    }

    static final String ACTIVE = "spring.profiles.active";
    static final String INCLUDE = "spring.profiles.include";
    static final String DEFAULT = "spring.profiles.default";
    static final String GROUP = "spring.profiles.group";
    static final String ON_PROFILE = "spring.config.activate.on-profile";
    private static final String ON_PROFILE_RELAXED = Names.relaxed(ON_PROFILE);
    /** Before Spring Boot 2.4: a YAML document's {@code spring.profiles}. */
    static final String LEGACY_ON_PROFILE = "spring.profiles";

    /** The keys whose values are profile names, for completion. */
    static boolean namesProfiles(String key) {
        String k = Names.relaxed(key).replaceAll("\\[\\d+]$", "");
        return k.equals(ACTIVE) || k.equals(INCLUDE) || k.equals(DEFAULT) || k.equals(ON_PROFILE_RELAXED)
            || k.equals(GROUP) || k.startsWith(GROUP + ".") || k.startsWith(GROUP + "[");
    }

    // ---- expressions

    /** Whether {@code @Profile}'s values match the active profiles: any one of them does. */
    static boolean matches(List<String> expressions, Set<String> active) {
        if (expressions.isEmpty()) {
            return true;
        }
        for (String e : expressions) {
            if (evaluate(e, active)) {
                return true;
            }
        }
        return false;
    }

    /**
     * Whether a profile expression holds for the active profiles: names, {@code !}, {@code &},
     * {@code |} and parentheses, {@code !} binding tightest and {@code &} before {@code |}. A
     * malformed expression holds (the bean is not hidden for a typo the compiler cannot see).
     */
    static boolean evaluate(String expression, Set<String> active) {
        try {
            Parser p = new Parser(expression, active);
            boolean v = p.or();
            p.skip();
            if (p.at < p.text.length()) {
                return true;
            }
            return v;
        } catch (IllegalArgumentException e) {
            return true;
        }
    }

    /** The profile names an expression mentions. */
    static List<String> names(String expression) {
        List<String> out = new ArrayList<>();
        for (String part : expression.split("[!&|()\\s]+")) {
            if (!part.isBlank()) {
                out.add(part.strip());
            }
        }
        return out;
    }

    private static final class Parser {
        final String text;
        final Set<String> active;
        int at;

        Parser(String text, Set<String> active) {
            this.text = text;
            this.active = active;
        }

        void skip() {
            while (at < text.length() && Character.isWhitespace(text.charAt(at))) {
                at++;
            }
        }

        boolean peek(char c) {
            skip();
            return at < text.length() && text.charAt(at) == c;
        }

        boolean or() {
            boolean v = and();
            while (peek('|')) {
                at++;
                v |= and();
            }
            return v;
        }

        boolean and() {
            boolean v = not();
            while (peek('&')) {
                at++;
                v &= not();
            }
            return v;
        }

        boolean not() {
            if (peek('!')) {
                at++;
                return !not();
            }
            if (peek('(')) {
                at++;
                boolean v = or();
                if (!peek(')')) {
                    throw new IllegalArgumentException("missing )");
                }
                at++;
                return v;
            }
            skip();
            int start = at;
            while (at < text.length() && "!&|() \t".indexOf(text.charAt(at)) < 0) {
                at++;
            }
            if (start == at) {
                throw new IllegalArgumentException("a profile name is missing");
            }
            return active.contains(text.substring(start, at));
        }
    }

    // ---- the application files

    /** One document of an application file: the profile it is for, if any, and its entries. */
    static final class Doc {
        String file;
        /** {@code spring.config.activate.on-profile} (or the file's own profile): an expression; null for every profile. */
        String onProfile;
        /** Its keys as written, and their values. */
        final Map<String, String> values = new LinkedHashMap<>();
    }

    /** What a module's application files say about profiles. */
    static final class Config {
        final List<Doc> docs = new ArrayList<>();
        /** Profile name to where it is mentioned: file names, documents, groups, the settings. */
        final Map<String, Set<String>> known = new TreeMap<>();

        List<String> list(String key) {
            List<String> out = new ArrayList<>();
            for (Doc d : docs) {
                if (d.onProfile == null) {
                    out.addAll(listValue(d.values, key));
                }
            }
            return out;
        }

        /** {@code spring.profiles.group.<name>}: the group's members. */
        Map<String, List<String>> groups() {
            Map<String, List<String>> out = new LinkedHashMap<>();
            for (Doc d : docs) {
                if (d.onProfile != null) {
                    continue;
                }
                for (String k : d.values.keySet()) {
                    if (Names.relaxed(k).startsWith(GROUP + ".") && k.length() > GROUP.length() + 1) {
                        String name = k.substring(GROUP.length() + 1).replaceAll("\\[\\d+]$", "");
                        out.computeIfAbsent(name, x -> new ArrayList<>());
                    }
                }
            }
            for (String g : out.keySet()) {
                out.get(g).addAll(list(GROUP + "." + g));
            }
            return out;
        }

        void know(String name, String where) {
            if (name != null && !name.isBlank()) {
                known.computeIfAbsent(name.strip(), k -> new LinkedHashSet<>()).add(where);
            }
        }
    }

    /** A key's value as a list: {@code a, b}, {@code [a, b]}, or YAML's {@code key[0]}, {@code key[1]}. */
    static List<String> listValue(Map<String, String> values, String key) {
        List<String> out = new ArrayList<>();
        String want = Names.relaxed(key);
        for (Map.Entry<String, String> e : values.entrySet()) {
            String k = Names.relaxed(e.getKey()).replaceAll("\\[\\d+]$", "");
            if (e.getValue() != null && k.equals(want)) {
                for (String part : e.getValue().replace("[", "").replace("]", "").split(",")) {
                    String p = PropsDoc.unquote(part.strip());
                    if (!p.isEmpty()) {
                        out.add(p);
                    }
                }
            }
        }
        return out;
    }

    /** The profile of an {@code application-<profile>.yml} file name, or null. */
    static String fileProfile(String fileName) {
        String n = fileName;
        int slash = Math.max(n.lastIndexOf('/'), n.lastIndexOf('\\'));
        n = n.substring(slash + 1);
        for (String base : List.of("application-", "bootstrap-")) {
            if (n.startsWith(base)) {
                int dot = n.lastIndexOf('.');
                String ext = dot < 0 ? "" : n.substring(dot).toLowerCase(Locale.ROOT);
                if (List.of(".properties", ".yml", ".yaml").contains(ext) && dot > base.length()) {
                    return n.substring(base.length(), dot);
                }
            }
        }
        return null;
    }

    /** The application files of these resource folders: {@code application.*} first, then each profile's. */
    static Config read(List<File> dirs) {
        Config c = new Config();
        for (File dir : dirs) {
            File[] files = dir.listFiles();
            if (files == null) {
                continue;
            }
            List<File> sorted = new ArrayList<>(List.of(files));
            sorted.sort((a, b) -> {
                boolean pa = fileProfile(a.getName()) != null;
                boolean pb = fileProfile(b.getName()) != null;
                return pa == pb ? a.getName().compareTo(b.getName()) : pa ? 1 : -1;
            });
            for (File f : sorted) {
                String name = f.getName();
                boolean base = name.matches("application\\.(properties|ya?ml)");
                String profile = fileProfile(name);
                if (!f.isFile() || (!base && (profile == null || !name.startsWith("application-")))) {
                    continue;
                }
                try {
                    String text = Files.readString(f.toPath(), StandardCharsets.UTF_8);
                    add(c, name, profile, PropsDoc.parse(text, !name.endsWith(".properties")));
                } catch (IOException e) {
                    // unreadable: skip
                }
            }
        }
        return c;
    }

    /** One application file's documents, of the profile its name gives (null for {@code application.*}). */
    static void add(Config c, String name, String profile, PropsDoc doc) {
        if (profile != null) {
            c.know(profile, name);
        }
        Map<Integer, Doc> docs = new TreeMap<>();
        for (PropsDoc.Entry e : doc.entries) {
            Doc d = docs.computeIfAbsent(e.document, i -> {
                Doc n = new Doc();
                n.file = name;
                n.onProfile = profile;
                return n;
            });
            String key = Names.relaxed(e.key);
            if (e.value != null) {
                d.values.put(e.key, e.value);
            }
            if ((key.equals(ON_PROFILE_RELAXED) || key.equals(LEGACY_ON_PROFILE)) && e.value != null) {
                String expr = PropsDoc.unquote(e.value.strip()).replace(",", "|");
                d.onProfile = profile == null ? expr : "(" + profile + ") & (" + expr + ")";
                for (String n : names(expr)) {
                    c.know(n, name + ", a document for " + expr);
                }
            }
        }
        for (Doc d : docs.values()) {
            c.docs.add(d);
            for (String key : List.of(ACTIVE, INCLUDE, DEFAULT)) {
                for (String n : listValue(d.values, key)) {
                    c.know(n, name + ", " + key);
                }
            }
        }
        for (Map.Entry<String, List<String>> g : c.groups().entrySet()) {
            c.know(g.getKey(), "the group " + g.getKey());
            for (String n : g.getValue()) {
                c.know(n, "the group " + g.getKey());
            }
        }
    }

    /**
     * The active profiles of a module: {@code chosen} when given (as {@code --spring.profiles.active}
     * would), else the files' {@code spring.profiles.active}; with what they include and what their
     * groups hold; else the default profiles.
     */
    static Set<String> active(Config c, List<String> chosen) {
        Set<String> out = new LinkedHashSet<>();
        for (String p : chosen != null ? chosen : c.list(ACTIVE)) {
            if (!p.isBlank()) {
                out.add(p.strip());
            }
        }
        out.addAll(c.list(INCLUDE));
        Map<String, List<String>> groups = c.groups();
        boolean grew = true;
        while (grew) {
            grew = false;
            for (String p : new ArrayList<>(out)) {
                for (String member : groups.getOrDefault(p, List.of())) {
                    grew |= out.add(member);
                }
            }
        }
        if (out.isEmpty()) {
            List<String> defaults = c.list(DEFAULT);
            out.addAll(defaults.isEmpty() ? List.of("default") : defaults);
        }
        return out;
    }

    /**
     * The values the active profiles see, by relaxed key, in Spring's order: the files' general
     * documents, then each active profile's.
     */
    static Map<String, String> values(Config c, Set<String> active) {
        Map<String, String> out = new LinkedHashMap<>();
        for (Doc d : c.docs) {
            if (d.onProfile == null) {
                d.values.forEach((k, v) -> out.put(Names.relaxed(k), v));
            }
        }
        for (Doc d : c.docs) {
            if (d.onProfile != null && evaluate(d.onProfile, active)) {
                d.values.forEach((k, v) -> out.put(Names.relaxed(k), v));
            }
        }
        return out;
    }
}
