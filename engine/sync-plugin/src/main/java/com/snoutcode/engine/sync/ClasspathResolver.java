// Copyright 2000-2026 JetBrains s.r.o. and contributors. Use of this source code is governed by the Apache 2.0 license.
// Adapted for SnoutCode from IntelliJ IDEA Community Edition,
// plugins/gradle/tooling-extension-impl/src/com/intellij/gradle/toolingExtension/impl/model/dependencyModel/GradleDependencyResolver.java
// and .../dependencyModel/auxiliary/LegacyAuxiliaryArtifactResolver.java.
// Changes: emits a flat, ordered classpath of project, module and file entries as plain maps;
// resolves source jars for all of a project's configurations in one artifact resolution query.
package com.snoutcode.engine.sync;

import org.gradle.api.Action;
import org.gradle.api.Project;
import org.gradle.api.artifacts.ArtifactCollection;
import org.gradle.api.artifacts.ArtifactView;
import org.gradle.api.artifacts.Configuration;
import org.gradle.api.artifacts.component.ComponentIdentifier;
import org.gradle.api.artifacts.component.ModuleComponentIdentifier;
import org.gradle.api.artifacts.component.ProjectComponentIdentifier;
import org.gradle.api.artifacts.result.ArtifactResult;
import org.gradle.api.artifacts.result.ComponentArtifactsResult;
import org.gradle.api.artifacts.result.DependencyResult;
import org.gradle.api.artifacts.result.ResolvedArtifactResult;
import org.gradle.api.artifacts.result.UnresolvedDependencyResult;
import org.gradle.jvm.JvmLibrary;
import org.gradle.language.base.artifact.SourcesArtifact;

import java.io.File;
import java.util.ArrayList;
import java.util.Collection;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

/**
 * Resolves configurations into classpath entries. Project dependencies stay project references
 * (with the files Gradle would put on the classpath for them); module dependencies carry their
 * coordinates, jar and, once {@link #attachSources} has run, their source jar.
 */
final class ClasspathResolver {
    private final Project project;
    private final Map<String, ModuleComponentIdentifier> modules = new LinkedHashMap<>();
    private final List<Map<String, Object>> moduleEntries = new ArrayList<>();

    ClasspathResolver(Project project) {
        this.project = project;
    }

    static final class Result {
        final List<Map<String, Object>> entries = new ArrayList<>();
        final List<Map<String, Object>> unresolved = new ArrayList<>();
    }

    Result resolve(Configuration configuration) {
        Result result = new Result();
        if (configuration == null) {
            return result;
        }
        Set<ResolvedArtifactResult> artifacts = artifacts(configuration);
        Map<String, Map<String, Object>> projects = new HashMap<>();
        Set<String> seenFiles = new LinkedHashSet<>();
        for (ResolvedArtifactResult artifact : artifacts) {
            File file = artifact.getFile();
            if (!seenFiles.add(file.getAbsolutePath())) {
                continue;
            }
            ComponentIdentifier id = artifact.getId().getComponentIdentifier();
            if (id instanceof ProjectComponentIdentifier) {
                ProjectComponentIdentifier pid = (ProjectComponentIdentifier) id;
                String build = buildPath(pid);
                String key = build + "|" + pid.getProjectPath();
                Map<String, Object> entry = projects.get(key);
                if (entry == null) {
                    entry = new LinkedHashMap<>();
                    entry.put("kind", "project");
                    entry.put("build", build);
                    entry.put("path", pid.getProjectPath());
                    entry.put("files", new ArrayList<String>());
                    projects.put(key, entry);
                    result.entries.add(entry);
                }
                @SuppressWarnings("unchecked")
                List<String> files = (List<String>) entry.get("files");
                files.add(file.getAbsolutePath());
            } else if (id instanceof ModuleComponentIdentifier) {
                ModuleComponentIdentifier mid = (ModuleComponentIdentifier) id;
                Map<String, Object> entry = new LinkedHashMap<>();
                entry.put("kind", "module");
                entry.put("group", mid.getGroup());
                entry.put("name", mid.getModule());
                entry.put("version", mid.getVersion());
                entry.put("file", file.getAbsolutePath());
                result.entries.add(entry);
                modules.put(mid.getDisplayName(), mid);
                moduleEntries.add(entry);
            } else {
                Map<String, Object> entry = new LinkedHashMap<>();
                entry.put("kind", "file");
                entry.put("file", file.getAbsolutePath());
                result.entries.add(entry);
            }
        }
        collectUnresolved(configuration, result);
        return result;
    }

    /** Lenient artifact view: a broken dependency must not hide the rest of the classpath. */
    private static Set<ResolvedArtifactResult> artifacts(Configuration configuration) {
        ArtifactView view = configuration.getIncoming().artifactView(new Action<ArtifactView.ViewConfiguration>() {
            @Override
            public void execute(ArtifactView.ViewConfiguration viewConfiguration) {
                viewConfiguration.setLenient(true);
            }
        });
        ArtifactCollection collection = view.getArtifacts();
        return collection.getArtifacts();
    }

    private static void collectUnresolved(Configuration configuration, Result result) {
        try {
            for (DependencyResult dependency : configuration.getIncoming().getResolutionResult().getAllDependencies()) {
                if (dependency instanceof UnresolvedDependencyResult) {
                    UnresolvedDependencyResult unresolved = (UnresolvedDependencyResult) dependency;
                    Map<String, Object> entry = new LinkedHashMap<>();
                    entry.put("requested", unresolved.getRequested().getDisplayName());
                    Throwable failure = unresolved.getFailure();
                    while (failure.getCause() != null && failure.getCause() != failure) {
                        failure = failure.getCause();
                    }
                    entry.put("reason", String.valueOf(failure.getMessage()));
                    result.unresolved.add(entry);
                }
            }
        } catch (RuntimeException e) {
            Map<String, Object> entry = new LinkedHashMap<>();
            entry.put("requested", configuration.getName());
            entry.put("reason", String.valueOf(e.getMessage()));
            result.unresolved.add(entry);
        }
    }

    /**
     * Looks up source jars for every module seen so far, in one query, and adds a
     * {@code sources} field to the matching entries. Returns how many modules have sources.
     */
    int attachSources() {
        if (modules.isEmpty()) {
            return 0;
        }
        Map<String, File> sources = new HashMap<>();
        Collection<ComponentIdentifier> ids = new ArrayList<ComponentIdentifier>(modules.values());
        @SuppressWarnings("unchecked")
        Set<ComponentArtifactsResult> components = project.getDependencies()
            .createArtifactResolutionQuery()
            .forComponents(ids)
            .withArtifacts(JvmLibrary.class, SourcesArtifact.class)
            .execute()
            .getResolvedComponents();
        for (ComponentArtifactsResult component : components) {
            for (ArtifactResult artifact : component.getArtifacts(SourcesArtifact.class)) {
                if (artifact instanceof ResolvedArtifactResult) {
                    sources.put(component.getId().getDisplayName(), ((ResolvedArtifactResult) artifact).getFile());
                    break;
                }
            }
        }
        for (Map<String, Object> entry : moduleEntries) {
            String key = entry.get("group") + ":" + entry.get("name") + ":" + entry.get("version");
            File src = sources.get(key);
            if (src != null) {
                entry.put("sources", src.getAbsolutePath());
            }
        }
        return sources.size();
    }

    /** The build that owns a project dependency: a build path on Gradle 8.2+, the build name before. */
    private static String buildPath(ProjectComponentIdentifier id) {
        Object build = Reflect.call(id, "getBuild");
        Object path = Reflect.call(build, "getBuildPath");
        if (path == null) {
            path = Reflect.call(build, "getName");
        }
        return path == null ? ":" : path.toString();
    }
}
