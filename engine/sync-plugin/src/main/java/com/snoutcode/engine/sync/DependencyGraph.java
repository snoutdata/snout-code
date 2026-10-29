package com.snoutcode.engine.sync;

import org.gradle.api.artifacts.Configuration;
import org.gradle.api.artifacts.Dependency;
import org.gradle.api.artifacts.ProjectDependency;
import org.gradle.api.artifacts.component.ComponentIdentifier;
import org.gradle.api.artifacts.component.ComponentSelector;
import org.gradle.api.artifacts.component.ModuleComponentIdentifier;
import org.gradle.api.artifacts.component.ModuleComponentSelector;
import org.gradle.api.artifacts.component.ProjectComponentIdentifier;
import org.gradle.api.artifacts.component.ProjectComponentSelector;
import org.gradle.api.artifacts.result.ComponentSelectionReason;
import org.gradle.api.artifacts.result.DependencyResult;
import org.gradle.api.artifacts.result.ResolvedComponentResult;
import org.gradle.api.artifacts.result.ResolvedDependencyResult;
import org.gradle.api.artifacts.result.UnresolvedDependencyResult;

import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Deque;
import java.util.IdentityHashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * A configuration's resolved dependency graph, for the Dependencies page of Project Structure:
 * the dependencies the configuration and those it extends declare (the root's edges, each with
 * the configuration that declares it), every component the resolution selected, once, with the
 * edges it brings in, and on each edge the version asked for when another one won.
 *
 * <pre>
 * { "roots": [Edge], "nodes": [ { "kind": "module" | "project" | "other", "group", "name",
 *   "version" | "build", "path" | "id", "reason"?: "conflict" | "forced" | "rule" | "constraint",
 *   "deps": [Edge] } ] }
 * Edge: { "to": node index, "requested"?: version asked for when not the one selected,
 *         "declaredIn"?: configuration (root edges) }
 *     | { "unresolved": "group:name:version", "reason" }
 * </pre>
 *
 * Constraints (platforms' and {@code constraints {}} entries) are not edges. Reading the graph
 * resolves nothing the classpaths did not: Gradle keeps the configuration's resolution result.
 */
final class DependencyGraph {
    /** System property ({@code -Dsnout.sync.dependencyGraphs=false}) that leaves the graphs out. */
    static final String PROPERTY = "snout.sync.dependencyGraphs";
    /** A graph stops growing here: a build with more is beyond what a page can show. */
    static final int MAX_NODES = 4000;

    private DependencyGraph() {
    }

    static boolean enabled() {
        return !"false".equals(System.getProperty(PROPERTY));
    }

    /** The graph of {@code configuration}, or null when it has none or cannot be resolved. */
    static Map<String, Object> read(Configuration configuration, List<String> errors) {
        if (configuration == null) {
            return null;
        }
        try {
            return graph(configuration);
        } catch (Throwable e) {
            errors.add("dependency graph of " + configuration.getName() + ": " + e);
            return null;
        }
    }

    private static Map<String, Object> graph(Configuration configuration) {
        Map<String, String> declaredIn = declarations(configuration);
        ResolvedComponentResult root = configuration.getIncoming().getResolutionResult().getRoot();
        Map<ResolvedComponentResult, Integer> index = new IdentityHashMap<>();
        List<Map<String, Object>> nodes = new ArrayList<>();
        Deque<ResolvedComponentResult> queue = new ArrayDeque<>();
        List<Object> roots = edges(root, root, index, nodes, queue, declaredIn);
        while (!queue.isEmpty()) {
            ResolvedComponentResult component = queue.removeFirst();
            Map<String, Object> node = nodes.get(index.get(component));
            node.put("deps", edges(component, root, index, nodes, queue, null));
        }
        if (roots.isEmpty()) {
            return null;
        }
        Map<String, Object> out = new LinkedHashMap<>();
        out.put("roots", roots);
        out.put("nodes", nodes);
        return out;
    }

    /** The edges out of {@code from}; components not seen yet are added and queued. */
    private static List<Object> edges(ResolvedComponentResult from, ResolvedComponentResult root,
                                      Map<ResolvedComponentResult, Integer> index, List<Map<String, Object>> nodes,
                                      Deque<ResolvedComponentResult> queue, Map<String, String> declaredIn) {
        List<Object> out = new ArrayList<>();
        for (DependencyResult dependency : from.getDependencies()) {
            if (Boolean.TRUE.equals(Reflect.call(dependency, "isConstraint"))) {
                continue;
            }
            Map<String, Object> edge = new LinkedHashMap<>();
            ComponentSelector requested = dependency.getRequested();
            if (dependency instanceof UnresolvedDependencyResult) {
                edge.put("unresolved", requested.getDisplayName());
                Throwable failure = ((UnresolvedDependencyResult) dependency).getFailure();
                while (failure != null && failure.getCause() != null && failure.getCause() != failure) {
                    failure = failure.getCause();
                }
                edge.put("reason", failure == null ? null : String.valueOf(failure.getMessage()));
            } else if (dependency instanceof ResolvedDependencyResult) {
                ResolvedComponentResult selected = ((ResolvedDependencyResult) dependency).getSelected();
                if (selected == root) {
                    continue; // the project itself (another variant of it), never a dependency
                }
                Integer to = index.get(selected);
                if (to == null) {
                    if (nodes.size() >= MAX_NODES) {
                        continue;
                    }
                    to = nodes.size();
                    index.put(selected, to);
                    nodes.add(node(selected));
                    queue.addLast(selected);
                }
                edge.put("to", to);
                if (requested instanceof ModuleComponentSelector
                    && selected.getId() instanceof ModuleComponentIdentifier) {
                    String asked = ((ModuleComponentSelector) requested).getVersion();
                    String got = ((ModuleComponentIdentifier) selected.getId()).getVersion();
                    if (!got.equals(asked)) {
                        edge.put("requested", asked);
                    }
                }
            } else {
                continue;
            }
            if (declaredIn != null) {
                String key = key(requested);
                if (key != null && declaredIn.containsKey(key)) {
                    edge.put("declaredIn", declaredIn.get(key));
                }
            }
            out.add(edge);
        }
        return out;
    }

    private static Map<String, Object> node(ResolvedComponentResult component) {
        Map<String, Object> node = new LinkedHashMap<>();
        ComponentIdentifier id = component.getId();
        if (id instanceof ModuleComponentIdentifier) {
            ModuleComponentIdentifier m = (ModuleComponentIdentifier) id;
            node.put("kind", "module");
            node.put("group", m.getGroup());
            node.put("name", m.getModule());
            node.put("version", m.getVersion());
        } else if (id instanceof ProjectComponentIdentifier) {
            ProjectComponentIdentifier p = (ProjectComponentIdentifier) id;
            node.put("kind", "project");
            node.put("build", buildPath(p));
            node.put("path", p.getProjectPath());
        } else {
            node.put("kind", "other");
            node.put("id", id.getDisplayName());
        }
        String reason = reason(component.getSelectionReason());
        if (reason != null) {
            node.put("reason", reason);
        }
        return node;
    }

    /** Why a component was selected, when it is worth saying: another version lost to it. */
    private static String reason(ComponentSelectionReason reason) {
        if (reason == null) {
            return null;
        }
        if (reason.isConflictResolution()) {
            return "conflict";
        }
        if (reason.isForced()) {
            return "forced";
        }
        if (reason.isSelectedByRule()) {
            return "rule";
        }
        if (Boolean.TRUE.equals(Reflect.call(reason, "isConstrained"))) {
            return "constraint";
        }
        return null;
    }

    /**
     * The configuration that declares each dependency of {@code configuration}'s hierarchy, by
     * {@code group:name} or {@code project:<path>}: the nearest one, as Gradle merges them.
     */
    private static Map<String, String> declarations(Configuration configuration) {
        Map<String, String> out = new LinkedHashMap<>();
        for (Configuration c : configuration.getHierarchy()) {
            for (Dependency d : c.getDependencies()) {
                String key;
                if (d instanceof ProjectDependency) {
                    Object path = Reflect.call(d, "getPath");
                    if (path == null) {
                        path = Reflect.call(Reflect.call(d, "getDependencyProject"), "getPath");
                    }
                    key = path == null ? null : "project:" + path;
                } else {
                    key = d.getGroup() == null ? null : d.getGroup() + ":" + d.getName();
                }
                if (key != null && !out.containsKey(key)) {
                    out.put(key, c.getName());
                }
            }
        }
        return out;
    }

    private static String key(ComponentSelector selector) {
        if (selector instanceof ModuleComponentSelector) {
            ModuleComponentSelector m = (ModuleComponentSelector) selector;
            return m.getGroup() + ":" + m.getModule();
        }
        if (selector instanceof ProjectComponentSelector) {
            return "project:" + ((ProjectComponentSelector) selector).getProjectPath();
        }
        return null;
    }

    /** The build that owns a project: a build path on Gradle 8.2+, the build name before. */
    private static String buildPath(ProjectComponentIdentifier id) {
        Object build = Reflect.call(id, "getBuild");
        Object path = Reflect.call(build, "getBuildPath");
        if (path == null) {
            path = Reflect.call(build, "getName");
        }
        return path == null ? ":" : path.toString();
    }
}
