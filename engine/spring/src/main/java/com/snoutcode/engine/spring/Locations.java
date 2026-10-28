package com.snoutcode.engine.spring;

import org.eclipse.core.runtime.IPath;
import org.eclipse.jdt.core.IClassFile;
import org.eclipse.jdt.core.IJavaElement;
import org.eclipse.jdt.core.IPackageFragmentRoot;
import org.eclipse.jdt.core.IType;
import org.eclipse.jdt.core.JavaModelException;
import org.eclipse.jdt.core.dom.IBinding;
import org.eclipse.jdt.core.dom.IMethodBinding;
import org.eclipse.jdt.core.dom.ITypeBinding;
import org.eclipse.jdt.core.dom.IVariableBinding;
import org.eclipse.jdt.ls.core.internal.JDTUtils;
import org.eclipse.jdt.ls.core.internal.ResourceUtils;
import org.eclipse.lsp4j.Location;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.stream.Stream;

/**
 * Where a binding is declared, anywhere: in a Java file (through JDT), in the Scala file a
 * Scala bridge class came from (through its {@code SourceFile} attribute and the text), or in a
 * library class file (a {@code jdt://} URI jdtls can open).
 */
final class Locations {
    /** Class folder of the Scala bridge to the Scala source folder attached to it. */
    private final Map<String, Path> bridges = new ConcurrentHashMap<>();
    private final Map<String, ScalaLocator> scalaFiles = new ConcurrentHashMap<>();
    private final Map<String, Path> sourceFileIndex = new ConcurrentHashMap<>();

    void bridge(String classFolder, Path sourceRoot) {
        bridges.put(classFolder, sourceRoot);
    }

    void forget(String uri) {
        scalaFiles.remove(uri);
    }

    void clearScala() {
        scalaFiles.clear();
        sourceFileIndex.clear();
    }

    Loc of(IBinding binding) {
        try {
            IJavaElement e = binding.getJavaElement();
            if (e == null) {
                return null;
            }
            ScalaLocator scala = scala(e);
            if (scala != null) {
                if (binding instanceof ITypeBinding t) {
                    return scala.type(simpleName(t));
                }
                ITypeBinding owner = binding instanceof IMethodBinding m ? m.getDeclaringClass()
                    : binding instanceof IVariableBinding v ? v.getDeclaringClass() : null;
                if (owner != null) {
                    return scala.member(simpleName(owner), binding.getName(), binding instanceof IMethodBinding);
                }
            }
            return toLoc(JDTUtils.toLocation(e));
        } catch (JavaModelException | RuntimeException e) {
            return null;
        }
    }

    Loc parameter(IMethodBinding m, int index) {
        try {
            IJavaElement e = m.getJavaElement();
            ScalaLocator scala = e == null ? null : scala(e);
            if (scala != null && m.isConstructor()) {
                String[] names = m.getParameterNames();
                if (names != null && index < names.length) {
                    Loc p = scala.parameter(simpleName(m.getDeclaringClass()), names[index]);
                    if (p != null) {
                        return p;
                    }
                }
                return scala.type(simpleName(m.getDeclaringClass()));
            }
        } catch (RuntimeException e) {
            // fall through to the method
        }
        return of(m);
    }

    static String simpleName(ITypeBinding t) {
        String binary = TypeScanner.binary(t);
        int i = Math.max(binary.lastIndexOf('.'), binary.lastIndexOf('$'));
        String name = binary.substring(i + 1);
        return name.isEmpty() ? t.getName() : name;
    }

    /** The Scala file of an element of a Scala bridge class, or null. */
    ScalaLocator scala(IJavaElement element) {
        IClassFile classFile = (IClassFile) element.getAncestor(IJavaElement.CLASS_FILE);
        IPackageFragmentRoot root = (IPackageFragmentRoot) element.getAncestor(IJavaElement.PACKAGE_FRAGMENT_ROOT);
        if (classFile == null || root == null) {
            return null;
        }
        IPath path = root.getPath();
        Path sources = bridges.get(path.toOSString());
        if (sources == null) {
            return null;
        }
        try {
            IType type = classFile instanceof org.eclipse.jdt.core.IOrdinaryClassFile o ? o.getType() : null;
            String pkg = type == null ? "" : type.getPackageFragment().getElementName();
            String fileName = ClassFiles.sourceFile(classFile.getBytes());
            if (fileName == null) {
                return null;
            }
            Path file = sources.resolve(pkg.replace('.', '/')).resolve(fileName);
            if (!Files.isRegularFile(file)) {
                file = find(sources, fileName);
            }
            if (file == null) {
                return null;
            }
            Path found = file;
            String uri = ResourceUtils.fixURI(found.toUri());
            return scalaFiles.computeIfAbsent(uri, u -> {
                try {
                    return new ScalaLocator(u, Files.readString(found, StandardCharsets.UTF_8));
                } catch (IOException ex) {
                    return new ScalaLocator(u, "");
                }
            });
        } catch (JavaModelException e) {
            return null;
        }
    }

    /** A Scala file outside its package's folder: Scala allows it. */
    private Path find(Path sources, String fileName) {
        String key = sources + "|" + fileName;
        Path cached = sourceFileIndex.get(key);
        if (cached != null) {
            return cached;
        }
        try (Stream<Path> files = Files.walk(sources)) {
            Path found = files.filter(p -> p.getFileName().toString().equals(fileName)).findFirst().orElse(null);
            if (found != null) {
                sourceFileIndex.put(key, found);
            }
            return found;
        } catch (IOException e) {
            return null;
        }
    }

    static Loc toLoc(Location l) {
        if (l == null || l.getRange() == null) {
            return null;
        }
        return new Loc(l.getUri(), l.getRange().getStart().getLine(), l.getRange().getStart().getCharacter(),
            l.getRange().getEnd().getLine(), l.getRange().getEnd().getCharacter());
    }
}
