package com.snoutcode.engine.cli;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import javax.tools.JavaCompiler;
import javax.tools.ToolProvider;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.stream.Stream;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;

class ScalaClassFolderTest {
    @TempDir
    Path dir;

    /** Compiles a class and makes its SourceFile attribute name {@code sourceFile} instead. */
    private void compiledClass(String name, String sourceFile) throws IOException {
        Path src = dir.resolve("javac/" + name + ".java");
        Files.createDirectories(src.getParent());
        Files.writeString(src, "package a.b; class " + name.replace("$", "") + " {}");
        JavaCompiler javac = ToolProvider.getSystemJavaCompiler();
        assertEquals(0, javac.run(null, null, null, "-d", dir.resolve("javac-out").toString(), src.toString()));
        byte[] bytes = Files.readAllBytes(dir.resolve("javac-out/a/b/" + name.replace("$", "") + ".class"));
        byte[] from = utf8(name + ".java");
        byte[] to = utf8(sourceFile);
        int at = indexOf(bytes, from);
        byte[] out = new byte[bytes.length - from.length + to.length];
        System.arraycopy(bytes, 0, out, 0, at);
        System.arraycopy(to, 0, out, at, to.length);
        System.arraycopy(bytes, at + from.length, out, at + to.length, bytes.length - at - from.length);
        Path target = dir.resolve("classes/a/b/" + name + ".class");
        Files.createDirectories(target.getParent());
        Files.write(target, out);
    }

    /** A CONSTANT_Utf8 entry's length and bytes. */
    private static byte[] utf8(String s) {
        byte[] text = s.getBytes(StandardCharsets.UTF_8);
        byte[] out = new byte[text.length + 2];
        out[0] = (byte) (text.length >> 8);
        out[1] = (byte) text.length;
        System.arraycopy(text, 0, out, 2, text.length);
        return out;
    }

    private static int indexOf(byte[] haystack, byte[] needle) {
        outer:
        for (int i = 0; i <= haystack.length - needle.length; i++) {
            for (int j = 0; j < needle.length; j++) {
                if (haystack[i + j] != needle[j]) {
                    continue outer;
                }
            }
            return i;
        }
        throw new AssertionError("not found");
    }

    private List<String> files(Path root) throws IOException {
        try (Stream<Path> walk = Files.walk(root)) {
            return walk.filter(Files::isRegularFile).map(p -> root.relativize(p).toString()).sorted().toList();
        }
    }

    @Test
    void mirrorsOnlyScalaClassesGroupedBySourceRoot() throws IOException {
        compiledClass("Foo", "Foo.scala");
        compiledClass("Foo$", "Foo.scala");
        compiledClass("Bar", "Bar.java");
        compiledClass("Baz", "Baz.scala");
        Path roots0 = dir.resolve("src0");
        Path roots1 = dir.resolve("src1");
        Files.createDirectories(roots0.resolve("a/b"));
        Files.createDirectories(roots1.resolve("a/b"));
        Files.writeString(roots1.resolve("a/b/Foo.scala"), "package a.b\nclass Foo\n");

        Path mirror = dir.resolve("mirror");
        List<ScalaClassFolder.Folder> folders = ScalaClassFolder.mirror(dir.resolve("classes"), mirror, List.of(roots0, roots1));
        assertEquals(List.of("1/a/b/Foo$.class", "1/a/b/Foo.class", "other/a/b/Baz.class"), files(mirror));
        assertEquals(2, folders.size());
        assertEquals(roots1, folders.get(0).sourceDir());
        assertEquals(2, folders.get(0).classes());
        assertNull(folders.get(1).sourceDir());

        Files.delete(dir.resolve("classes/a/b/Foo$.class"));
        Files.delete(dir.resolve("classes/a/b/Baz.class"));
        ScalaClassFolder.mirror(dir.resolve("classes"), mirror, List.of(roots0, roots1));
        assertEquals(List.of("1/a/b/Foo.class"), files(mirror));
    }

    @Test
    void findsAScalaFileOutsideItsPackageFolderByName() throws IOException {
        compiledClass("Qux", "Qux.scala");
        compiledClass("Twice", "Twice.scala");
        Path roots0 = dir.resolve("src0");
        Path roots1 = dir.resolve("src1");
        // Scala lets a file declare package a.b from any folder.
        Files.createDirectories(roots1.resolve("elsewhere"));
        Files.writeString(roots1.resolve("elsewhere/Qux.scala"), "package a.b\nclass Qux\n");
        Files.createDirectories(roots0.resolve("x"));
        Files.writeString(roots0.resolve("x/Twice.scala"), "package a.b\nclass Twice\n");
        Files.writeString(roots1.resolve("elsewhere/Twice.scala"), "package a.b\nclass Twice\n");

        Path mirror = dir.resolve("mirror");
        ScalaClassFolder.mirror(dir.resolve("classes"), mirror, List.of(roots0, roots1));
        assertEquals(List.of("1/a/b/Qux.class", "other/a/b/Twice.class"), files(mirror),
            "a name found in one root only goes to that root");
    }

    @Test
    void readsTheSourceFileAttribute() throws IOException {
        compiledClass("Bar", "Bar.java");
        assertEquals("Bar.java", ScalaClassFolder.sourceFile(Files.readAllBytes(dir.resolve("classes/a/b/Bar.class"))));
        assertNull(ScalaClassFolder.sourceFile(new byte[] {0, 1, 2, 3, 4, 5, 6, 7, 8, 9}));
    }
}
