package com.snoutcode.engine.cli;

import java.io.ByteArrayInputStream;
import java.io.DataInputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TreeMap;
import java.util.stream.Stream;

/**
 * The Scala half of a joint compile's output. Gradle's Scala compile task writes the classes of
 * the Java files it compiled jointly into the same folder; jdtls already has those as sources,
 * and the duplicates break its searches. This keeps mirrors of the folder with only the classes
 * whose {@code SourceFile} attribute names a {@code .scala} file, hard linked where the file
 * system allows.
 */
final class ScalaClassFolder {
    private ScalaClassFolder() {
    }

    /** One mirror folder: Scala classes whose sources lie under {@link #sourceDir} (null: found nowhere). */
    record Folder(Path classesDir, Path sourceDir, int classes) {
    }

    /**
     * Brings the mirrors under {@code mirrorRoot} in line with the Scala classes of {@code classes}.
     * Classes are grouped by the source root that holds their {@code .scala} file, one mirror per
     * root ({@code 0}, {@code 1}, ...; {@code other} for files found in none), so each can carry
     * its source root as the source attachment: jdtls then shows the Scala source for a Scala type
     * instead of decompiling it.
     */
    static List<Folder> mirror(Path classes, Path mirrorRoot, List<Path> sourceRoots) throws IOException {
        Map<String, Set<Path>> wanted = new TreeMap<>();
        ScalaFilesByName byName = new ScalaFilesByName(sourceRoots);
        List<Path> files;
        try (Stream<Path> walk = Files.walk(classes)) {
            files = walk.filter(p -> p.toString().endsWith(".class")).toList();
        }
        for (Path file : files) {
            String source;
            try (InputStream in = Files.newInputStream(file)) {
                source = sourceFile(in.readAllBytes());
            }
            if (source == null || !source.endsWith(".scala")) {
                continue;
            }
            Path relative = classes.relativize(file);
            Path packageDir = relative.getParent();
            String group = "other";
            for (int i = 0; i < sourceRoots.size(); i++) {
                Path root = sourceRoots.get(i);
                if (Files.isRegularFile((packageDir == null ? root : root.resolve(packageDir.toString())).resolve(source))) {
                    group = Integer.toString(i);
                    break;
                }
            }
            if (group.equals("other")) {
                // Scala does not tie a file's folder to its package: find the file by name, when
                // exactly one root has a file of that name.
                int only = byName.rootOf(source);
                if (only >= 0) {
                    group = Integer.toString(only);
                }
            }
            wanted.computeIfAbsent(group, g -> new HashSet<>()).add(relative);
            link(file, mirrorRoot.resolve(group).resolve(relative.toString()));
        }
        prune(mirrorRoot, wanted);
        List<Folder> out = new ArrayList<>();
        for (Map.Entry<String, Set<Path>> e : wanted.entrySet()) {
            Path sourceDir = e.getKey().equals("other") ? null : sourceRoots.get(Integer.parseInt(e.getKey()));
            out.add(new Folder(mirrorRoot.resolve(e.getKey()), sourceDir, e.getValue().size()));
        }
        return out;
    }

    /** The roots that hold a {@code .scala} file of a given name, read once, when first asked. */
    private static final class ScalaFilesByName {
        private final List<Path> roots;
        private Map<String, Set<Integer>> index;

        ScalaFilesByName(List<Path> roots) {
            this.roots = roots;
        }

        /** The one root holding a file of this name, or -1 for none or several. */
        int rootOf(String fileName) throws IOException {
            if (index == null) {
                index = new HashMap<>();
                for (int i = 0; i < roots.size(); i++) {
                    if (!Files.isDirectory(roots.get(i))) {
                        continue;
                    }
                    int root = i;
                    try (Stream<Path> walk = Files.walk(roots.get(i))) {
                        walk.filter(p -> p.toString().endsWith(".scala"))
                            .forEach(p -> index.computeIfAbsent(p.getFileName().toString(), k -> new HashSet<>()).add(root));
                    }
                }
            }
            Set<Integer> found = index.get(fileName);
            return found != null && found.size() == 1 ? found.iterator().next() : -1;
        }
    }

    private static void link(Path file, Path target) throws IOException {
        if (Files.exists(target) && (Files.isSameFile(file, target)
            || (Files.size(file) == Files.size(target)
                && Files.getLastModifiedTime(file).equals(Files.getLastModifiedTime(target))))) {
            return;
        }
        Files.createDirectories(target.getParent());
        Files.deleteIfExists(target);
        try {
            Files.createLink(target, file);
        } catch (IOException | UnsupportedOperationException e) {
            Files.copy(file, target, StandardCopyOption.COPY_ATTRIBUTES);
        }
    }

    /** Deletes mirrored files that are no longer wanted, and the folders left empty. */
    private static void prune(Path mirrorRoot, Map<String, Set<Path>> wanted) throws IOException {
        if (!Files.isDirectory(mirrorRoot)) {
            Files.createDirectories(mirrorRoot);
            return;
        }
        try (Stream<Path> walk = Files.walk(mirrorRoot)) {
            for (Path p : walk.sorted(Comparator.reverseOrder()).toList()) {
                if (p.equals(mirrorRoot)) {
                    continue;
                }
                Path relative = mirrorRoot.relativize(p);
                if (Files.isRegularFile(p)) {
                    Set<Path> group = wanted.get(relative.getName(0).toString());
                    if (relative.getNameCount() < 2 || group == null
                        || !group.contains(relative.subpath(1, relative.getNameCount()))) {
                        Files.delete(p);
                    }
                } else if (Files.isDirectory(p)) {
                    try (Stream<Path> children = Files.list(p)) {
                        if (children.findAny().isEmpty()) {
                            Files.delete(p);
                        }
                    }
                }
            }
        }
    }

    /** The {@code SourceFile} attribute of a class file, or null when it has none or is not one. */
    static String sourceFile(byte[] bytes) throws IOException {
        DataInputStream in = new DataInputStream(new ByteArrayInputStream(bytes));
        if (in.readInt() != 0xCAFEBABE) {
            return null;
        }
        in.skipBytes(4); // minor, major
        int count = in.readUnsignedShort();
        String[] utf8 = new String[count];
        for (int i = 1; i < count; i++) {
            int tag = in.readUnsignedByte();
            switch (tag) {
                case 1 -> utf8[i] = in.readUTF();
                case 3, 4, 9, 10, 11, 12, 17, 18 -> in.skipBytes(4);
                case 5, 6 -> {
                    in.skipBytes(8);
                    i++;
                }
                case 7, 8, 16, 19, 20 -> in.skipBytes(2);
                case 15 -> in.skipBytes(3);
                default -> {
                    return null;
                }
            }
        }
        in.skipBytes(6); // access, this, super
        in.skipBytes(2 * in.readUnsignedShort()); // interfaces
        for (int members = 0; members < 2; members++) { // fields, then methods
            int n = in.readUnsignedShort();
            for (int i = 0; i < n; i++) {
                in.skipBytes(6);
                skipAttributes(in);
            }
        }
        int attributes = in.readUnsignedShort();
        for (int i = 0; i < attributes; i++) {
            int name = in.readUnsignedShort();
            int length = in.readInt();
            if ("SourceFile".equals(utf8[name])) {
                return utf8[in.readUnsignedShort()];
            }
            in.skipBytes(length);
        }
        return null;
    }

    private static void skipAttributes(DataInputStream in) throws IOException {
        int n = in.readUnsignedShort();
        for (int i = 0; i < n; i++) {
            in.skipBytes(2);
            in.skipBytes(in.readInt());
        }
    }
}
