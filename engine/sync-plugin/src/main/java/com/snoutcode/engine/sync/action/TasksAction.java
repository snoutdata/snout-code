package com.snoutcode.engine.sync.action;

import org.gradle.tooling.BuildAction;
import org.gradle.tooling.BuildController;
import org.gradle.tooling.model.DomainObjectSet;
import org.gradle.tooling.model.GradleProject;
import org.gradle.tooling.model.GradleTask;
import org.gradle.tooling.model.gradle.GradleBuild;

import java.io.File;
import java.io.Serializable;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Lists the tasks of every project of the root build and its included builds, with their group
 * and description, from Gradle's own {@link GradleProject} model. Configures the builds (every
 * task is realized) but runs nothing.
 */
public final class TasksAction implements BuildAction<TasksAction.Result> {
    private static final long serialVersionUID = 1L;

    /** The tasks, per build and project. Plain data, so the client needs no Gradle model classes. */
    public static final class Result implements Serializable {
        private static final long serialVersionUID = 1L;
        public final List<Build> builds = new ArrayList<>();
    }

    public static final class Build implements Serializable {
        private static final long serialVersionUID = 1L;
        public String name;
        public String rootDir;
        /** {@code root} or {@code included}. */
        public String kind;
        public final List<Project> projects = new ArrayList<>();
        public final List<String> errors = new ArrayList<>();
    }

    public static final class Project implements Serializable {
        private static final long serialVersionUID = 1L;
        public String path;
        public String name;
        public String projectDir;
        public final List<Task> tasks = new ArrayList<>();
    }

    public static final class Task implements Serializable {
        private static final long serialVersionUID = 1L;
        public String name;
        public String path;
        public String group;
        public String description;
        public boolean isPublic;
    }

    @Override
    public Result execute(BuildController controller) {
        Result result = new Result();
        GradleBuild root = controller.getBuildModel();
        Map<String, GradleBuild> builds = new LinkedHashMap<>();
        collect(root, builds);
        for (GradleBuild build : builds.values()) {
            File rootDir = build.getBuildIdentifier().getRootDir();
            if (build != root && "buildSrc".equals(rootDir.getName())) {
                continue; // buildSrc's tasks cannot be run from the root build
            }
            Build out = new Build();
            out.rootDir = rootDir.getAbsolutePath();
            out.name = build.getRootProject().getName();
            out.kind = build == root ? "root" : "included";
            try {
                GradleProject project = controller.getModel(build.getRootProject(), GradleProject.class);
                addProjects(project, out.projects);
            } catch (RuntimeException e) {
                out.errors.add(String.valueOf(e.getMessage()));
            }
            result.builds.add(out);
        }
        return result;
    }

    private static void addProjects(GradleProject project, List<Project> out) {
        Project p = new Project();
        p.path = project.getPath();
        p.name = project.getName();
        try {
            p.projectDir = project.getProjectDirectory().getAbsolutePath();
        } catch (RuntimeException e) {
            // Gradle before 2.4
        }
        for (GradleTask task : project.getTasks()) {
            Task t = new Task();
            t.name = task.getName();
            t.path = task.getPath();
            t.description = task.getDescription();
            try {
                t.group = task.getGroup();
                t.isPublic = task.isPublic();
            } catch (RuntimeException e) {
                // Gradle before 2.5
            }
            p.tasks.add(t);
        }
        out.add(p);
        for (GradleProject child : project.getChildren()) {
            addProjects(child, out);
        }
    }

    private static void collect(GradleBuild build, Map<String, GradleBuild> out) {
        String key = build.getBuildIdentifier().getRootDir().getAbsolutePath();
        if (out.containsKey(key)) {
            return;
        }
        out.put(key, build);
        List<GradleBuild> children = new ArrayList<>();
        try {
            DomainObjectSet<? extends GradleBuild> editable = build.getEditableBuilds();
            children.addAll(editable);
        } catch (RuntimeException e) {
            // Gradle before 4.10
        }
        try {
            children.addAll(build.getIncludedBuilds());
        } catch (RuntimeException e) {
            // Gradle before 3.3
        }
        for (GradleBuild child : children) {
            collect(child, out);
        }
    }
}
