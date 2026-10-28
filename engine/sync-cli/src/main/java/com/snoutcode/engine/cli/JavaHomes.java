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
 * <p>A daemon must run on a Java its Gradle version supports (Gradle's compatibility matrix:
 * 21 from Gradle 8.5, 17 from 7.3, 16 from 7.0, and so on): Gradle older than 6.7 gets Java 11,
 * newer Gradle the Java its build's toolchain names, else Java 17, each capped at the newest Java
 * that Gradle version runs on. A preferred JDK (the {@code --java-home} option) wins whenever the
 * build's Gradle can run on it and the build names no other Java. A build is never run on a JDK
 * its Gradle cannot run on: when none is installed, it needs the newest LTS Java that fits.
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
     * system and user JDK folders of macOS and Linux, SDKMAN, {@code ~/.jdks}, Gradle's
     * provisioned toolchains, and SnoutCode's own: the JDKs the app downloads, the engine's build
     * JDK, and the JDK folders under the cache directory.
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
        List<Path> snout = snoutJdkFolders(home, cacheDir);
        parents.addAll(snout);
        Path gradleJdks = home.resolve(".gradle/jdks");
        for (Path parent : parents) {
            // SnoutCode's folders, and Gradle's provisioned toolchains
            // (jdks/eclipse_adoptium-17-aarch64-os_x.2/jdk-17.0.20.1+1), may hold a JDK one level
            // deeper: an archive's own top folder.
            addJdks(parent, candidates, snout.contains(parent) || parent.equals(gradleJdks) ? 2 : 1);
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

    /**
     * SnoutCode's JDK folders: where the app downloads JDKs ({@code ~/Library/Application
     * Support/SnoutCode/jdks} on macOS), the engine's build JDK and JDKs in the default cache
     * directory ({@code ~/Library/Caches/SnoutCode/build-jdk}, {@code .../jdks}), and the same in
     * another cache directory when one is given.
     */
    static List<Path> snoutJdkFolders(Path home, Path cacheDir) {
        List<Path> out = new ArrayList<>();
        String os = System.getProperty("os.name", "").toLowerCase(Locale.ROOT);
        if (os.contains("mac")) {
            out.add(home.resolve("Library/Application Support/SnoutCode/jdks"));
        } else if (os.contains("win")) {
            String appData = System.getenv("APPDATA");
            out.add((appData != null ? Path.of(appData) : home.resolve("AppData/Roaming")).resolve("SnoutCode/jdks"));
        } else {
            String data = System.getenv("XDG_DATA_HOME");
            out.add((data != null && !data.isEmpty() ? Path.of(data) : home.resolve(".local/share")).resolve("SnoutCode/jdks"));
        }
        Path defaultCache = Lsp.defaultCacheDir();
        for (Path cache : cacheDir == null || cacheDir.equals(defaultCache) ? List.of(defaultCache) : List.of(defaultCache, cacheDir)) {
            out.add(cache.resolve("build-jdk"));
            out.add(cache.resolve("jdks"));
        }
        return out;
    }

    private static void addJdks(Path parent, List<File> candidates, int depth) {
        if (depth <= 0 || !Files.isDirectory(parent)) {
            return;
        }
        try (Stream<Path> list = Files.list(parent)) {
            for (Path p : list.sorted().toList()) {
                if (!Files.isDirectory(p)) {
                    continue;
                }
                candidates.add(p.toFile());
                candidates.add(p.resolve("Contents/Home").toFile());
                if (read(p.toFile()) == null && read(p.resolve("Contents/Home").toFile()) == null) {
                    addJdks(p, candidates, depth - 1);
                }
            }
        } catch (IOException e) {
            // an unreadable folder holds no JDK we can use
        }
    }

    /** The oldest Java a Gradle version's daemon runs on: 8, and 17 from Gradle 9. */
    static int minJava(String gradleVersion) {
        int[] v = gradleVersion(gradleVersion);
        return v != null && v[0] >= 9 ? 17 : 8;
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
        int min = minJava(gradleVersion);
        int wanted = wantedJava(gradleVersion, scriptLevel);
        int floor = Math.max(min, scriptLevel == null ? 0 : scriptLevel);
        int[] v = gradleVersion(gradleVersion);
        boolean old = v != null && compare(v, 6, 7) < 0;
        if (preferred != null && preferred.feature() <= max && preferred.feature() >= floor
            && (!old || preferred.feature() <= 11)) {
            return preferred;
        }
        List<Jdk> fits = new ArrayList<>();
        for (Jdk jdk : installed) {
            if (jdk.feature() <= max && jdk.feature() >= min) {
                fits.add(jdk);
            }
        }
        // Distance from the wanted Java, newer before older at the same distance; then newest update.
        fits.sort(Comparator.<Jdk>comparingInt(j -> j.feature() >= wanted ? 2 * (j.feature() - wanted) : 2 * (wanted - j.feature()) + 1)
            .thenComparing(Jdk::version, Comparator.reverseOrder()));
        return fits.isEmpty() ? null : fits.get(0);
    }

    /**
     * The Java major a build needs and the machine lacks, when no installed JDK runs its Gradle
     * version: the newest LTS release (8, 11, 17, 21, 25) its Gradle runs on, not newer than the
     * Java it wants; null when a JDK does run it (or the version is unknown, so the Tooling API
     * picks). Gradle 8.1.1 needs 17, 7.0.2 needs 11, 5.2.1 needs 11.
     */
    public static Integer needs(String gradleVersion, Integer scriptLevel, Jdk chosen) {
        if (chosen != null || gradleVersion(gradleVersion) == null) {
            return null;
        }
        return lts(gradleVersion, wantedJava(gradleVersion, scriptLevel));
    }

    private static final int[] LTS = {8, 11, 17, 21, 25};

    /** The newest LTS Java a Gradle version runs on that is not newer than {@code wanted}. */
    static int lts(String gradleVersion, int wanted) {
        int min = minJava(gradleVersion);
        int max = maxJava(gradleVersion);
        int best = -1;
        for (int lts : LTS) {
            if (lts >= min && lts <= max && (lts <= wanted || best < 0)) {
                best = lts;
            }
        }
        return best > 0 ? best : wanted;
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
