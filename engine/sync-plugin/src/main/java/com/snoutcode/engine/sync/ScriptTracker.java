package com.snoutcode.engine.sync;

import org.gradle.api.Action;
import org.gradle.api.Project;
import org.gradle.api.invocation.Gradle;

import java.io.File;
import java.lang.reflect.InvocationHandler;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.WeakHashMap;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * Records every Gradle script the build loads, and which project was being configured when it
 * was loaded. That is how {@code apply from: "../shared.gradle"} files are found, so the IDE
 * knows which files should trigger a resync.
 *
 * <p>Gradle has no public API for this. The tracker listens on Gradle's internal
 * {@code ScriptExecutionListener} through a dynamic proxy, so a missing or changed interface
 * just disables it; {@link #scanBuildFile} is the fallback.
 */
final class ScriptTracker {
    private static final String[] LISTENER_CLASSES = {
        "org.gradle.internal.scripts.ScriptExecutionListener",
        "org.gradle.groovy.scripts.internal.ScriptExecutionListener",
    };

    private static final Map<Gradle, ScriptTracker> TRACKERS =
        Collections.synchronizedMap(new WeakHashMap<Gradle, ScriptTracker>());

    /** Matches {@code apply from: 'x'}, {@code apply(from: "x")} and Kotlin's {@code apply(from = "x")}. */
    private static final Pattern APPLY_FROM =
        Pattern.compile("apply\\s*\\(?\\s*from\\s*[:=]\\s*(?:file\\(\\s*)?['\"]([^'\"$]+)['\"]");

    private final Set<String> all = new LinkedHashSet<>();
    private final Set<String> beforeProjects = new LinkedHashSet<>();
    private final Map<String, Set<String>> byProject = new LinkedHashMap<>();
    private volatile String currentProject;
    private volatile boolean listening;

    static void install(Gradle gradle) {
        ScriptTracker tracker = new ScriptTracker();
        TRACKERS.put(gradle, tracker);
        tracker.listening = tracker.addListener(gradle);
        gradle.beforeProject(new Action<Project>() {
            @Override
            public void execute(Project project) {
                tracker.currentProject = project.getPath();
            }
        });
        gradle.afterProject(new Action<Project>() {
            @Override
            public void execute(Project project) {
                tracker.currentProject = null;
            }
        });
    }

    static ScriptTracker of(Gradle gradle) {
        return TRACKERS.get(gradle);
    }

    boolean isListening() {
        return listening;
    }

    private boolean addListener(Gradle gradle) {
        ClassLoader loader = Gradle.class.getClassLoader();
        for (String name : LISTENER_CLASSES) {
            Class<?> type;
            try {
                type = Class.forName(name, false, loader);
            } catch (Throwable e) {
                continue;
            }
            try {
                Object listener = Proxy.newProxyInstance(type.getClassLoader(), new Class<?>[] {type},
                    new InvocationHandler() {
                        @Override
                        public Object invoke(Object proxy, Method method, Object[] args) {
                            if (method.getDeclaringClass() == Object.class) {
                                return objectMethod(proxy, method, args);
                            }
                            if (args != null && args.length > 0) {
                                onScript(args[0]);
                            }
                            return null;
                        }
                    });
                gradle.addListener(listener);
                return true;
            } catch (Throwable e) {
                return false;
            }
        }
        return false;
    }

    private static Object objectMethod(Object proxy, Method method, Object[] args) {
        switch (method.getName()) {
            case "equals":
                return proxy == args[0];
            case "hashCode":
                return System.identityHashCode(proxy);
            default:
                return "SnoutScriptTracker";
        }
    }

    private void onScript(Object scriptSource) {
        File file = Reflect.file(Reflect.call(Reflect.call(scriptSource, "getResource"), "getFile"));
        if (file == null) {
            Object fileName = Reflect.call(scriptSource, "getFileName");
            if (fileName != null) {
                file = new File(fileName.toString());
            }
        }
        if (file == null || !file.isFile()) {
            return;
        }
        String path = file.getAbsolutePath();
        synchronized (this) {
            all.add(path);
            String project = currentProject;
            if (project == null) {
                beforeProjects.add(path);
            } else {
                Set<String> scripts = byProject.get(project);
                if (scripts == null) {
                    scripts = new LinkedHashSet<>();
                    byProject.put(project, scripts);
                }
                scripts.add(path);
            }
        }
    }

    /** Scripts loaded while the project was configured, including its own build file. */
    synchronized List<String> scriptsOf(String projectPath) {
        Set<String> scripts = byProject.get(projectPath);
        return scripts == null ? Collections.<String>emptyList() : new ArrayList<>(scripts);
    }

    /** Init and settings scripts: loaded before any project was configured. */
    synchronized List<String> scriptsBeforeProjects() {
        return new ArrayList<>(beforeProjects);
    }

    synchronized List<String> allScripts() {
        return new ArrayList<>(all);
    }

    /** Fallback when the listener is unavailable: the literal {@code apply from} paths in a build file. */
    static List<String> scanBuildFile(File buildFile) {
        List<String> out = new ArrayList<>();
        if (buildFile == null || !buildFile.isFile()) {
            return out;
        }
        String text;
        try {
            text = new String(java.nio.file.Files.readAllBytes(buildFile.toPath()), "UTF-8");
        } catch (Exception e) {
            return out;
        }
        Matcher m = APPLY_FROM.matcher(text);
        while (m.find()) {
            String ref = m.group(1);
            if (ref.contains("://")) {
                out.add(ref);
                continue;
            }
            File f = new File(ref);
            if (!f.isAbsolute()) {
                f = new File(buildFile.getParentFile(), ref);
            }
            try {
                out.add(f.getCanonicalPath());
            } catch (Exception e) {
                out.add(f.getAbsolutePath());
            }
        }
        return out;
    }
}
