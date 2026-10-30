package com.snoutcode.engine.cli;

import java.io.File;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayDeque;
import java.util.Deque;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * The Java a Maven build needs, read from its POMs' text before any sync (as
 * {@link JavaHomes#scriptLevel} reads Gradle scripts): the newest {@code release}, else
 * {@code source}, that the compiler plugin's configuration or the {@code maven.compiler.*} and
 * {@code java.version} properties of the reactor's POMs name. A JDK of that level is chosen to
 * compile the build.
 */
final class MavenLevels {
    private static final Pattern LEVEL = Pattern.compile(
        "<(?:maven\\.compiler\\.release|maven\\.compiler\\.source|release|source|java\\.version)>\\s*(?:1\\.)?(\\d+)\\s*</");
    private static final Pattern MODULE = Pattern.compile("<module>\\s*([^<]+?)\\s*</module>");
    private static final int[] LTS = {8, 11, 17, 21, 25};

    private MavenLevels() {
    }

    /** The newest Java level the reactor's POMs name; 8 when they name none. */
    static int level(File root) {
        int best = 0;
        Deque<Path> queue = new ArrayDeque<>();
        queue.add(root.toPath().resolve("pom.xml"));
        Set<Path> seen = new LinkedHashSet<>();
        while (!queue.isEmpty() && seen.size() < 500) {
            Path pom = queue.removeFirst().normalize();
            if (!seen.add(pom) || !Files.isRegularFile(pom)) {
                continue;
            }
            try {
                String text = Files.readString(pom).replaceAll("(?s)<!--.*?-->", "");
                Matcher m = LEVEL.matcher(text);
                while (m.find()) {
                    int v = Integer.parseInt(m.group(1));
                    if (v >= 5 && v < 100) {
                        best = Math.max(best, v);
                    }
                }
                Matcher mod = MODULE.matcher(text);
                while (mod.find()) {
                    Path p = pom.getParent().resolve(mod.group(1).trim());
                    queue.add(Files.isDirectory(p) ? p.resolve("pom.xml") : p);
                }
            } catch (IOException | RuntimeException e) {
                // unreadable: no level
            }
        }
        return best == 0 ? 8 : best;
    }

    /**
     * The JDK for a build of this level: the preferred one when it is of that level, else one
     * installed of that level, else the preferred one when it is newer, else the oldest newer one
     * installed; null when none is.
     */
    static JavaHomes.Jdk choose(int level, JavaHomes.Jdk preferred, List<JavaHomes.Jdk> installed) {
        if (preferred != null && preferred.feature() == level) {
            return preferred;
        }
        for (JavaHomes.Jdk jdk : installed) {
            if (jdk.feature() == level) {
                return jdk;
            }
        }
        if (preferred != null && preferred.feature() >= level) {
            return preferred;
        }
        JavaHomes.Jdk best = null;
        for (JavaHomes.Jdk jdk : installed) {
            if (jdk.feature() >= level && (best == null || jdk.feature() < best.feature())) {
                best = jdk;
            }
        }
        return best;
    }

    /** The LTS at or above a level: the JDK to download when none is installed. */
    static int lts(int level) {
        for (int l : LTS) {
            if (l >= level) {
                return l;
            }
        }
        return level;
    }

    /** A short reason for a Maven build that did not sync. */
    static String reason(String error) {
        if (error == null) {
            return "The Maven build did not sync";
        }
        String first = error.split("\n")[0].trim();
        Matcher m = Pattern.compile("Could not (?:find|transfer) artifact ([^\\s]+)").matcher(error);
        if (m.find()) {
            return "could not resolve " + m.group(1);
        }
        if (first.startsWith("IOException: ")) {
            first = first.substring("IOException: ".length());
        }
        return first.length() > 200 ? first.substring(0, 197) + "..." : first;
    }
}
