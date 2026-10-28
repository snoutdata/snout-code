// Copyright 2000-2023 JetBrains s.r.o. and contributors. Use of this source code is governed by the Apache 2.0 license.
// Adapted for SnoutCode from IntelliJ IDEA Community Edition,
// plugins/gradle/tooling-extension-api/src/com/intellij/gradle/toolingExtension/util/GradleReflectionUtil.java.
// Changes: lenient lookups that return null instead of throwing, Groovy dynamic property access,
// and unwrapping of Gradle providers and file types.
package com.snoutcode.engine.sync;

import org.codehaus.groovy.runtime.InvokerHelper;

import java.io.File;
import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.Collection;
import java.util.Collections;
import java.util.List;

/**
 * Version-tolerant access to Gradle objects. The plugin is compiled against a recent Gradle API
 * but runs in builds back to Gradle 5, so anything that is not in every version goes through here.
 */
final class Reflect {
    private Reflect() {
    }

    static boolean hasMethod(Object target, String methodName, Class<?>... parameterTypes) {
        if (target == null) {
            return false;
        }
        try {
            target.getClass().getMethod(methodName, parameterTypes);
            return true;
        } catch (NoSuchMethodException | SecurityException e) {
            return false;
        }
    }

    /** Calls a public no-argument method; null if it does not exist or throws. */
    static Object call(Object target, String methodName) {
        if (target == null) {
            return null;
        }
        try {
            Method method = target.getClass().getMethod(methodName);
            return method.invoke(target);
        } catch (Throwable e) {
            return null;
        }
    }

    /** Calls a public one-argument method; null if it does not exist or throws. */
    static Object call(Object target, String methodName, Class<?> parameterType, Object argument) {
        if (target == null) {
            return null;
        }
        try {
            Method method = target.getClass().getMethod(methodName, parameterType);
            return method.invoke(target, argument);
        } catch (Throwable e) {
            return null;
        }
    }

    /**
     * Reads a Groovy/Gradle dynamic property, which covers extensions, conventions and plain
     * getters alike (for example {@code sourceSet.scala} before and after Gradle 7.1).
     */
    static Object property(Object target, String name) {
        if (target == null) {
            return null;
        }
        try {
            return InvokerHelper.getProperty(target, name);
        } catch (Throwable e) {
            return null;
        }
    }

    /** Unwraps a Gradle {@code Provider} (lazy property) without failing; other values pass through. */
    static Object unwrap(Object value) {
        if (value == null) {
            return null;
        }
        if (isInstance(value, "org.gradle.api.provider.Provider")) {
            return call(value, "getOrNull");
        }
        return value;
    }

    /** Converts File, Directory, RegularFile, Path or a provider of one into a File. */
    static File file(Object value) {
        value = unwrap(value);
        if (value == null) {
            return null;
        }
        if (value instanceof File) {
            return (File) value;
        }
        if (value instanceof java.nio.file.Path) {
            return ((java.nio.file.Path) value).toFile();
        }
        Object asFile = call(value, "getAsFile");
        if (asFile instanceof File) {
            return (File) asFile;
        }
        return null;
    }

    static String string(Object value) {
        value = unwrap(value);
        return value == null ? null : value.toString();
    }

    static List<String> strings(Object value) {
        value = unwrap(value);
        if (!(value instanceof Iterable)) {
            return Collections.emptyList();
        }
        List<String> out = new ArrayList<>();
        for (Object o : (Iterable<?>) value) {
            if (o != null) {
                out.add(o.toString());
            }
        }
        return out;
    }

    static boolean isInstance(Object object, String className) {
        if (object == null) {
            return false;
        }
        for (Class<?> c = object.getClass(); c != null; c = c.getSuperclass()) {
            if (c.getName().equals(className)) {
                return true;
            }
            if (implementsInterface(c, className)) {
                return true;
            }
        }
        return false;
    }

    private static boolean implementsInterface(Class<?> c, String className) {
        for (Class<?> i : c.getInterfaces()) {
            if (i.getName().equals(className) || implementsInterface(i, className)) {
                return true;
            }
        }
        return false;
    }

    /** The class a Gradle-generated subclass ({@code Foo_Decorated}, {@code Foo$Inject}) stands for. */
    static Class<?> declaredType(Object object) {
        Class<?> c = object.getClass();
        while ((c.getName().endsWith("_Decorated") || c.getName().endsWith("$Inject")) && c.getSuperclass() != null) {
            c = c.getSuperclass();
        }
        return c;
    }

    static List<String> paths(Collection<File> files) {
        List<String> out = new ArrayList<>(files.size());
        for (File f : files) {
            out.add(f.getAbsolutePath());
        }
        return out;
    }
}
