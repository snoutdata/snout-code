package com.snoutcode.engine.cli;

import java.io.File;
import java.io.IOException;
import java.io.Reader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Collection;
import java.util.Comparator;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Properties;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import java.util.stream.Stream;

/**
 * The JDKs on this machine, and the one each Gradle build's daemon should run on.
 *
 * <p>A daemon must run on a Java its Gradle version supports: Gradle older than 6.7 gets Java 11,
 * newer Gradle the Java its build's toolchain names, else Java 17, each capped at the newest Java
 * that Gradle version runs on. A preferred JDK (the {@code --java-home} option) wins whenever the
 * build's Gradle can run on it and the build names no other Java.
 */
public final class JavaHomes {
    /** An installed JDK. */
    public record Jdk(File home, int feature, String version) {
    }

    private JavaHomes() {
    }

    /** The JDK at a Java home, from its {@code release} file; null if it is not one. */
    public static Jdk read(File home) {
        File dir = canonical(home);
        if (!new File(dir, "bin/java").isFile() && !new File(dir, "bin/java.exe").isFile()) {
            return null;
        }
        Properties release = new Properties();
        try (Reader r = Files.newBufferedReader(dir.toPath().resolve("release"), StandardCharsets.UTF_8)) {
            release.load(r);
        } catch (IOException e) {
            return null;
        }
        String version = release.getProperty("JAVA_VERSION", "").replace("\"", "").trim();
        int feature = feature(version);
        return feature <= 0 ? null : new Jdk(dir, feature, version);
    }

    /** {@code 1.8.0_402} is 8, {@code 17.0.2} is 17. */
    static int feature(String version) {
        Matcher m = Pattern.compile("^(?:1\\.)?(\\d+)").matcher(version);
        return m.find() ? Integer.parseInt(m.group(1)) : -1;
    }

    /**
     * The JDKs found in the usual places: the extra homes given, this JVM, {@code JAVA_HOME}, the
     * system and user JDK folders of macOS and Linux, SDKMAN, IntelliJ's {@code ~/.jdks}, Gradle's
     * provisioned toolchains, and SnoutCode's own JDK folders under the cache directory.
     */
    public static List<Jdk> discover(Collection<File> extra, Path cacheDir) {
        List<File> candidates = new ArrayList<>(extra);
        candidates.add(new File(System.getProperty("java.home")));
        String javaHome = System.getenv("JAVA_HOME");
        if (javaHome != null && !javaHome.isEmpty()) {
            candidates.add(new File(javaHome));
        }
        Path home = Path.of(System.getProperty("user.home"));
        List<Path> parents = new ArrayList<>(List.of(
            Path.of("/Library/Java/JavaVirtualMachines"), home.resolve("Library/Java/JavaVirtualMachines"),
            Path.of("/usr/lib/jvm"), Path.of("/usr/java"), Path.of("/opt/java"),
            home.resolve(".sdkman/candidates/java"), home.resolve(".jdks"), home.resolve(".gradle/jdks"),
            home.resolve(".asdf/installs/java")));
        if (cacheDir != null) {
            parents.add(cacheDir.resolve("jdks"));
            parents.add(cacheDir.resolve("build-jdk"));
        }
        for (Path parent : parents) {
            if (!Files.isDirectory(parent)) {
                continue;
            }
            try (Stream<Path> list = Files.list(parent)) {
                for (Path p : list.sorted().toList()) {
                    candidates.add(p.toFile());
                    candidates.add(p.resolve("Contents/Home").toFile());
                }
            } catch (IOException e) {
                // an unreadable folder holds no JDK we can use
            }
        }
        Map<File, Jdk> found = new LinkedHashMap<>();
        for (File candidate : candidates) {
            Jdk jdk = read(candidate);
            if (jdk != null) {
                found.putIfAbsent(jdk.home(), jdk);
            }
        }
        return new ArrayList<>(found.values());
    }

    /** The newest Java a Gradle version's daemon runs on, from Gradle's compatibility matrix. */
    static int maxJava(String gradleVersion) {
        int[] v = gradleVersion(gradleVersion);
        if (v == null) {
            return 21;
        }
        int[][] table = {
            {5, 0, 11}, {6, 0, 13}, {6, 3, 14}, {6, 7, 15}, {7, 0, 16}, {7, 3, 17}, {7, 5, 18}, {7, 6, 19},
            {8, 3, 20}, {8, 5, 21}, {8, 8, 22}, {8, 10, 23}, {8, 14, 24}, {9, 1, 25},
        };
        int max = 8;
        for (int[] row : table) {
            if (compare(v, row[0], row[1]) >= 0) {
                max = row[2];
            }
        }
        return max;
    }

    /**
     * The Java a build wants its daemon on: 11 for Gradle older than 6.7 (nothing newer runs
     * those builds reliably), else the toolchain or source level the build script names when it
     * is newer than 17, else 17; never newer than the Gradle version supports.
     */
    static int wantedJava(String gradleVersion, Integer scriptLevel) {
        int max = maxJava(gradleVersion);
        int[] v = gradleVersion(gradleVersion);
        if (v != null && compare(v, 6, 7) < 0) {
            return Math.min(11, max);
        }
        int wanted = scriptLevel != null && scriptLevel > 17 ? scriptLevel : 17;
        return Math.min(wanted, max);
    }

    /**
     * The JDK for a build's daemon: the preferred one when the Gradle version runs on it and it is
     * at least the build's script level, else the installed JDK closest to the wanted Java among
     * those the Gradle version runs on (the wanted one, then newer ones, then older ones).
     * Null when nothing fits; the caller then lets the Tooling API use its own JVM.
     */
    public static Jdk choose(String gradleVersion, Integer scriptLevel, Jdk preferred, List<Jdk> installed) {
        int max = maxJava(gradleVersion);
        int wanted = wantedJava(gradleVersion, scriptLevel);
        int floor = scriptLevel == null ? 0 : scriptLevel;
        int[] v = gradleVersion(gradleVersion);
        boolean old = v != null && compare(v, 6, 7) < 0;
        if (preferred != null && preferred.feature() <= max && preferred.feature() >= floor
            && (!old || preferred.feature() <= 11)) {
            return preferred;
        }
        List<Jdk> fits = new ArrayList<>();
        for (Jdk jdk : installed) {
            if (jdk.feature() <= max) {
                fits.add(jdk);
            }
        }
        // Distance from the wanted Java, newer before older at the same distance; then newest update.
        fits.sort(Comparator.<Jdk>comparingInt(j -> j.feature() >= wanted ? 2 * (j.feature() - wanted) : 2 * (wanted - j.feature()) + 1)
            .thenComparing(Jdk::version, Comparator.reverseOrder()));
        return fits.isEmpty() ? null : fits.get(0);
    }

    /** The Gradle version of a build's wrapper, or null when it has none. */
    public static String wrapperVersion(File buildRoot) {
        Path props = buildRoot.toPath().resolve("gradle/wrapper/gradle-wrapper.properties");
        if (!Files.isRegularFile(props)) {
            return null;
        }
        Properties p = new Properties();
        try (Reader r = Files.newBufferedReader(props, StandardCharsets.ISO_8859_1)) {
            p.load(r);
        } catch (IOException e) {
            return null;
        }
        Matcher m = Pattern.compile("gradle-([0-9][^/]*?)-(?:bin|all)\\.zip").matcher(p.getProperty("distributionUrl", ""));
        return m.find() ? m.group(1) : null;
    }

    private static final Pattern[] LEVEL_PATTERNS = {
        Pattern.compile("JavaLanguageVersion\\s*\\.\\s*of\\s*\\(\\s*['\"]?(\\d+)"),
        Pattern.compile("jvmToolchain\\s*\\(?\\s*(\\d+)"),
        Pattern.compile("(?:source|target)Compatibility\\s*=\\s*(?:JavaVersion\\.VERSION_|['\"])?(?:1[._])?(\\d+)"),
        Pattern.compile("options\\s*\\.\\s*release\\s*(?:=|\\.set\\s*\\()\\s*(\\d+)"),
    };

    /**
     * The newest Java level the build scripts at the root of a build name, from a toolchain,
     * {@code jvmToolchain}, a source or target compatibility or a {@code --release}; null if none.
     * A plain text scan: it runs before Gradle, to pick the daemon's JDK.
     */
    public static Integer scriptLevel(File buildRoot) {
        Integer level = null;
        for (String name : Discovery.BUILD_FILES) {
            Path file = buildRoot.toPath().resolve(name);
            if (!Files.isRegularFile(file)) {
                continue;
            }
            String text;
            try {
                text = Files.readString(file, StandardCharsets.UTF_8);
            } catch (IOException e) {
                continue;
            }
            text = text.replaceAll("(?s)/\\*.*?\\*/", "").replaceAll("(?m)//.*$", "");
            for (Pattern pattern : LEVEL_PATTERNS) {
                Matcher m = pattern.matcher(text);
                while (m.find()) {
                    int n = Integer.parseInt(m.group(1));
                    if (n >= 5 && n < 100 && (level == null || n > level)) {
                        level = n;
                    }
                }
            }
        }
        return level;
    }

    static int[] gradleVersion(String version) {
        if (version == null) {
            return null;
        }
        Matcher m = Pattern.compile("^(\\d+)\\.(\\d+)").matcher(version.trim().toLowerCase(Locale.ROOT));
        return m.find() ? new int[] {Integer.parseInt(m.group(1)), Integer.parseInt(m.group(2))} : null;
    }

    private static int compare(int[] v, int major, int minor) {
        return v[0] != major ? Integer.compare(v[0], major) : Integer.compare(v[1], minor);
    }

    private static File canonical(File f) {
        try {
            return f.getCanonicalFile();
        } catch (IOException e) {
            return f.getAbsoluteFile();
        }
    }
}
