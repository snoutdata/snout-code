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
 * Finds the Gradle builds of a folder, the way an IDE links several Gradle projects into one
 * project.
 *
 * <ul>
 *   <li>A folder that is itself a build, or lies inside one, is that one build: the nearest
 *       folder at or above it with a settings script, else the nearest with a build script.</li>
 *   <li>Otherwise every build under the folder: a folder with a settings script, or with a build
 *       script and no settings script above it inside the folder. A build owns everything below
 *       it (subprojects, included builds, buildSrc), so the search does not descend into one.</li>
 *   <li>{@code build}, {@code .gradle}, {@code node_modules} and hidden folders are skipped.</li>
 * </ul>
 */
public final class Discovery {
    static final List<String> SETTINGS = List.of("settings.gradle", "settings.gradle.kts");
    static final List<String> BUILD_FILES = List.of("build.gradle", "build.gradle.kts");
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

    private static void scan(Path dir, int depth, List<File> out) throws IOException {
        if (hasAny(dir, SETTINGS) || hasAny(dir, BUILD_FILES)) {
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
