package com.snoutcode.engine.cli;

import java.io.File;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Set;
import java.util.stream.Stream;

/**
 * Finds the Gradle and Maven builds of a folder, the way an IDE links several projects into one
 * project.
 *
 * <ul>
 *   <li>A folder that is itself a build, or lies inside one, is that one build: for Gradle the
 *       nearest folder at or above it with a settings script, else the nearest with a build
 *       script; for Maven the outermost POM of the reactor that holds it (each POM above that
 *       lists the one below among its modules).</li>
 *   <li>Otherwise every build under the folder: a folder with a settings script, or with a build
 *       script and no settings script above it inside the folder, or with a {@code pom.xml}. A
 *       build owns everything below it (subprojects, included builds, buildSrc, Maven modules),
 *       so the search does not descend into one. A folder with both Gradle scripts and a POM is
 *       a Gradle build.</li>
 *   <li>{@code build}, {@code target}, {@code .gradle}, {@code node_modules} and hidden folders
 *       are skipped.</li>
 * </ul>
 */
public final class Discovery {
    static final List<String> SETTINGS = List.of("settings.gradle", "settings.gradle.kts");
    static final List<String> BUILD_FILES = List.of("build.gradle", "build.gradle.kts");
    static final String POM = "pom.xml";
    private static final java.util.regex.Pattern MODULE = java.util.regex.Pattern.compile("<module>\\s*([^<]+?)\\s*</module>");
    private static final Set<String> SKIPPED = Set.of("build", "node_modules", "out", "target", "buildSrc");
    /** Deep enough for any sensible layout, shallow enough that opening a home folder ends. */
    static final int MAX_DEPTH = 8;

    private Discovery() {
    }

    /** The root directories of the builds the folder holds, sorted by path. */
    public static List<File> builds(File folder) throws IOException {
        Path dir = folder.getCanonicalFile().toPath();
        Path enclosing = enclosingBuild(dir);
        if (enclosing != null) {
            return List.of(enclosing.toFile());
        }
        Path reactor = enclosingReactor(dir);
        if (reactor != null) {
            return List.of(reactor.toFile());
        }
        List<File> out = new ArrayList<>();
        scan(dir, 0, out);
        out.sort((a, b) -> a.getPath().compareTo(b.getPath()));
        return out;
    }

    /**
     * The build the folder is or lies inside: the nearest folder at or above it with a settings
     * script, else the nearest with a build script. The search stops below the user's home and the
     * file system root, so a stray script there cannot capture every folder, and at a folder the
     * scan skips ({@code build}, {@code node_modules}, a hidden one...): what lies in a build's
     * outputs is not part of it.
     */
    static Path enclosingBuild(Path dir) {
        Path home = Path.of(System.getProperty("user.home")).toAbsolutePath().normalize();
        Path nearestBuildFile = null;
        for (Path p = dir; p != null && p.getParent() != null && !p.equals(home); p = p.getParent()) {
            if (hasAny(p, SETTINGS)) {
                return p;
            }
            if (nearestBuildFile == null && hasAny(p, BUILD_FILES)) {
                nearestBuildFile = p;
            }
            if (!searched(p)) {
                break; // a folder under build/ or node_modules/ is not part of the build above it
            }
        }
        return nearestBuildFile;
    }

    /** Whether a build's root is a Maven build: it has a POM and no Gradle script. */
    public static boolean isMaven(File root) {
        Path dir = root.toPath();
        return Files.isRegularFile(dir.resolve(POM)) && !hasAny(dir, SETTINGS) && !hasAny(dir, BUILD_FILES);
    }

    /**
     * The Maven reactor the folder is or lies inside: the nearest folder at or above it with a
     * POM, then up through each parent folder whose POM lists the one below as a module. Stops
     * below the user's home, as {@link #enclosingBuild} does.
     */
    static Path enclosingReactor(Path dir) {
        Path home = Path.of(System.getProperty("user.home")).toAbsolutePath().normalize();
        Path found = null;
        for (Path p = dir; p != null && p.getParent() != null && !p.equals(home); p = p.getParent()) {
            if (Files.isRegularFile(p.resolve(POM))) {
                found = p;
                break;
            }
            if (!searched(p)) {
                return null;
            }
        }
        if (found == null) {
            return null;
        }
        for (Path up = found.getParent(); up != null && up.getParent() != null && !up.equals(home); up = up.getParent()) {
            if (!listsModule(up, found)) {
                break;
            }
            found = up;
        }
        return found;
    }

    /** Whether the POM in {@code parent} lists {@code child} among its modules (in any profile). */
    static boolean listsModule(Path parent, Path child) {
        Path pom = parent.resolve(POM);
        if (!Files.isRegularFile(pom)) {
            return false;
        }
        try {
            java.util.regex.Matcher m = MODULE.matcher(Files.readString(pom).replaceAll("(?s)<!--.*?-->", ""));
            while (m.find()) {
                Path module = parent.resolve(m.group(1).trim()).normalize();
                if (module.equals(child) || module.equals(child.resolve(POM))) {
                    return true;
                }
            }
        } catch (IOException | RuntimeException e) {
            return false;
        }
        return false;
    }

    private static void scan(Path dir, int depth, List<File> out) throws IOException {
        if (hasAny(dir, SETTINGS) || hasAny(dir, BUILD_FILES) || Files.isRegularFile(dir.resolve(POM))) {
            out.add(dir.toFile());
            return;
        }
        if (depth >= MAX_DEPTH) {
            return;
        }
        List<Path> children;
        try (Stream<Path> list = Files.list(dir)) {
            children = list.filter(Discovery::searched).sorted().toList();
        } catch (IOException e) {
            return; // an unreadable folder holds no build we can sync
        }
        for (Path child : children) {
            scan(child, depth + 1, out);
        }
    }

    private static boolean searched(Path p) {
        String name = p.getFileName().toString();
        return Files.isDirectory(p) && !Files.isSymbolicLink(p) && !name.startsWith(".") && !SKIPPED.contains(name);
    }

    static boolean hasAny(Path dir, List<String> names) {
        for (String name : names) {
            if (Files.isRegularFile(dir.resolve(name))) {
                return true;
            }
        }
        return false;
    }
}
