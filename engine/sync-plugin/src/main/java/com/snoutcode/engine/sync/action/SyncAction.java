package com.snoutcode.engine.sync.action;

import com.snoutcode.engine.sync.model.SnoutProjectModel;
import org.gradle.tooling.BuildAction;
import org.gradle.tooling.BuildController;
import org.gradle.tooling.model.DomainObjectSet;
import org.gradle.tooling.model.gradle.BasicGradleProject;
import org.gradle.tooling.model.gradle.GradleBuild;
import org.gradle.tooling.model.build.BuildEnvironment;

import java.io.File;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * The one Tooling API call of a sync: runs inside the daemon, walks the root build, its included
 * builds and (Gradle 8+) buildSrc, and asks every project for its {@link SnoutProjectModel}.
 * It only configures the builds; no task runs.
 */
public final class SyncAction implements BuildAction<SyncResult> {
    private static final long serialVersionUID = 1L;

    @Override
    public SyncResult execute(BuildController controller) {
        SyncResult result = new SyncResult();
        try {
            BuildEnvironment env = controller.getModel(BuildEnvironment.class);
            result.gradleVersion = env.getGradle().getGradleVersion();
            result.gradleJavaHome = env.getJava().getJavaHome().getAbsolutePath();
        } catch (RuntimeException e) {
            // Older Gradle versions may not serve BuildEnvironment from inside an action.
        }

        GradleBuild root = controller.getBuildModel();
        Map<String, GradleBuild> builds = new LinkedHashMap<>();
        collect(root, builds);
        for (GradleBuild build : builds.values()) {
            SyncResult.Build out = new SyncResult.Build();
            File rootDir = build.getBuildIdentifier().getRootDir();
            out.rootDir = rootDir.getAbsolutePath();
            out.name = build.getRootProject().getName();
            if (build == root) {
                out.kind = "root";
            } else if ("buildSrc".equals(rootDir.getName())) {
                out.kind = "buildSrc";
            } else {
                out.kind = "included";
            }
            for (BasicGradleProject project : build.getProjects()) {
                try {
                    SnoutProjectModel model = controller.findModel(project, SnoutProjectModel.class);
                    if (model == null) {
                        out.errors.add(project.getPath() + ": no model (is the sync plugin applied?)");
                    } else {
                        out.projectJson.add(model.getJson());
                    }
                } catch (RuntimeException e) {
                    out.errors.add(project.getPath() + ": " + describe(e));
                }
            }
            result.builds.add(out);
        }
        return result;
    }

    /** The root build plus every build of the composite, depth first, each once. */
    private static void collect(GradleBuild build, Map<String, GradleBuild> out) {
        String key = build.getBuildIdentifier().getRootDir().getAbsolutePath();
        if (out.containsKey(key)) {
            return;
        }
        out.put(key, build);
        List<GradleBuild> children = new ArrayList<>();
        try {
            // Gradle 4.10+: every build in the composite, including nested included builds and,
            // from Gradle 8.0, buildSrc.
            DomainObjectSet<? extends GradleBuild> editable = build.getEditableBuilds();
            children.addAll(editable);
        } catch (RuntimeException e) {
            // Unsupported by this Gradle version; fall back to direct included builds.
        }
        try {
            children.addAll(build.getIncludedBuilds());
        } catch (RuntimeException e) {
            // Unsupported by this Gradle version.
        }
        for (GradleBuild child : children) {
            collect(child, out);
        }
    }

    private static String describe(Throwable e) {
        StringBuilder sb = new StringBuilder(String.valueOf(e.getMessage()));
        for (Throwable c = e.getCause(); c != null && c != c.getCause(); c = c.getCause()) {
            sb.append(" | caused by: ").append(c.getMessage());
        }
        return sb.toString();
    }
}
