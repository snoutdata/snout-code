package com.snoutcode.engine.jdtls;

import org.eclipse.core.resources.IFolder;
import org.eclipse.core.resources.IProject;
import org.eclipse.core.resources.IProjectDescription;
import org.eclipse.core.resources.IResource;
import org.eclipse.core.resources.IWorkspace;
import org.eclipse.core.resources.IWorkspaceRoot;
import org.eclipse.core.resources.ResourcesPlugin;
import org.eclipse.core.runtime.CoreException;
import org.eclipse.core.runtime.IPath;
import org.eclipse.core.runtime.IProgressMonitor;
import org.eclipse.core.runtime.QualifiedName;
import org.eclipse.core.runtime.SubMonitor;
import org.eclipse.jdt.core.IClasspathAttribute;
import org.eclipse.jdt.core.IClasspathEntry;
import org.eclipse.jdt.core.IJavaProject;
import org.eclipse.jdt.core.JavaCore;
import org.eclipse.jdt.ls.core.internal.JavaLanguageServerPlugin;

import java.io.File;
import java.net.URI;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Set;

/**
 * Applies an {@link ImportPlan} to the Eclipse workspace.
 *
 * <p>The projects live in jdtls's workspace data directory, not in the user's repository: every
 * source root is a linked folder, and the output folders, {@code .project}, {@code .classpath}
 * and {@code .settings} are all under the workspace project's own location.
 */
final class WorkspaceLayout {
    /** Output folders, {@code bin/<sourceSet>}, inside the workspace project. */
    static final String OUTPUT_FOLDER = "bin";

    /** JDT's hidden project that links class folders outside the workspace. */
    private static final String EXTERNAL_FOLDERS_PROJECT = ".org.eclipse.jdt.core.external.folders";

    /** The Gradle root directory a project was imported from, to tell stale projects apart. */
    static final QualifiedName ROOT_DIR = new QualifiedName(SnoutNature.BUNDLE_ID, "rootDir");

    private WorkspaceLayout() {
    }

    static void apply(ImportPlan plan, IProgressMonitor monitor) throws CoreException {
        IWorkspace workspace = ResourcesPlugin.getWorkspace();
        IWorkspaceRoot root = workspace.getRoot();
        SubMonitor progress = SubMonitor.convert(monitor, "Importing " + plan.rootDir.getName(), plan.projects.size() * 10 + 2);

        Set<String> wanted = new LinkedHashSet<>();
        for (ImportPlan.Project spec : plan.projects) {
            wanted.add(spec.name);
        }
        // Projects of ours from an earlier import of this root that the build no longer has.
        for (IProject project : root.getProjects()) {
            if (SnoutNature.has(project) && !wanted.contains(project.getName())
                && plan.rootDir.getPath().equals(project.getPersistentProperty(ROOT_DIR))) {
                project.delete(false, true, progress.split(0)); // only the workspace copy and links
            }
        }
        progress.worked(1);

        // Every project exists before any classpath refers to it, or JDT marks the reference missing.
        for (ImportPlan.Project spec : plan.projects) {
            create(workspace, spec, progress.split(3));
        }
        for (ImportPlan.Project spec : plan.projects) {
            configure(workspace, plan, spec, progress.split(7));
        }
        refreshExternalFolders(root, progress.split(1));
    }

    /**
     * JDT reaches class folders outside the workspace (the Scala bridge's) through links in a
     * hidden project, and reads them through the resource tree, which does not see files that
     * appeared since the last session: a Scala recompile adds classes to a folder JDT already
     * knows. Refreshing the links makes JDT see the folders as they are now.
     */
    private static void refreshExternalFolders(IWorkspaceRoot root, IProgressMonitor monitor) {
        IProject external = root.getProject(EXTERNAL_FOLDERS_PROJECT);
        if (!external.exists()) {
            return;
        }
        try {
            external.refreshLocal(IResource.DEPTH_INFINITE, monitor);
        } catch (CoreException e) {
            JavaLanguageServerPlugin.logException("SnoutCode: could not refresh the external class folders", e);
        }
    }

    private static void create(IWorkspace workspace, ImportPlan.Project spec, IProgressMonitor monitor) throws CoreException {
        SubMonitor progress = SubMonitor.convert(monitor, 3);
        IProject project = workspace.getRoot().getProject(spec.name);
        if (!project.exists()) {
            // No location: the project directory is <workspace>/<name>, out of the repository.
            project.create(workspace.newProjectDescription(spec.name), progress.split(1));
        }
        if (!project.isOpen()) {
            project.open(progress.split(1));
        }
        IProjectDescription description = project.getDescription();
        String[] natures = {JavaCore.NATURE_ID, SnoutNature.ID};
        if (!Arrays.equals(description.getNatureIds(), natures)) {
            description.setNatureIds(natures);
            project.setDescription(description, progress.split(1));
        }
    }

    private static void configure(IWorkspace workspace, ImportPlan plan, ImportPlan.Project spec, IProgressMonitor monitor)
        throws CoreException {
        SubMonitor progress = SubMonitor.convert(monitor, 3);
        IProject project = workspace.getRoot().getProject(spec.name);
        project.setPersistentProperty(ROOT_DIR, plan.rootDir.getPath());
        project.setPersistentProperty(new QualifiedName(SnoutNature.BUNDLE_ID, "gradlePath"),
            spec.buildName + " " + spec.gradlePath);
        if (spec.java.encoding != null && !spec.java.encoding.equals(project.getDefaultCharset(false))) {
            project.setDefaultCharset(spec.java.encoding, progress.split(1));
        }

        linkSourceFolders(project, spec, progress.split(1));

        IJavaProject javaProject = JavaCore.create(project);
        Jdks.applyCompilerOptions(javaProject, spec.java);
        List<IClasspathEntry> classpath = new ArrayList<>();
        Map<String, IPath> outputs = new HashMap<>();
        for (ImportPlan.SourceRoot source : spec.sources) {
            IPath output = outputs.computeIfAbsent(source.sourceSet, name -> outputFolder(project, name));
            List<IClasspathAttribute> attributes = new ArrayList<>();
            if (source.test) {
                attributes.add(JavaCore.newClasspathAttribute(IClasspathAttribute.TEST, "true"));
            }
            if (source.generated) {
                attributes.add(JavaCore.newClasspathAttribute(IClasspathAttribute.OPTIONAL, "true"));
                attributes.add(JavaCore.newClasspathAttribute(IClasspathAttribute.IGNORE_OPTIONAL_PROBLEMS, "true"));
            }
            IPath[] exclusions = source.exclusions.stream().map(IPath::fromPortableString).toArray(IPath[]::new);
            classpath.add(JavaCore.newSourceEntry(project.getFolder(source.link).getFullPath(), new IPath[0],
                exclusions, output, attributes.toArray(new IClasspathAttribute[0])));
        }
        classpath.add(Jdks.containerEntry(spec.java));
        for (ImportPlan.Dependency dependency : spec.dependencies) {
            IClasspathAttribute[] attributes = dependency.test
                ? new IClasspathAttribute[] {JavaCore.newClasspathAttribute(IClasspathAttribute.TEST, "true")}
                : new IClasspathAttribute[0];
            if (dependency.project != null) {
                List<IClasspathAttribute> projectAttributes = new ArrayList<>(Arrays.asList(attributes));
                // Main code of a dependent project must not see this project's test code.
                projectAttributes.add(JavaCore.newClasspathAttribute(IClasspathAttribute.WITHOUT_TEST_CODE, "true"));
                classpath.add(JavaCore.newProjectEntry(IPath.fromOSString("/" + dependency.project), null, false,
                    projectAttributes.toArray(new IClasspathAttribute[0]), false));
            } else {
                IPath sources = dependency.sources == null ? null : IPath.fromOSString(dependency.sources.getAbsolutePath());
                classpath.add(JavaCore.newLibraryEntry(IPath.fromOSString(dependency.file.getAbsolutePath()), sources,
                    null, null, attributes, dependency.exported));
            }
        }
        IPath defaultOutput = outputFolder(project, "default");
        IClasspathEntry[] entries = classpath.toArray(new IClasspathEntry[0]);
        if (!Arrays.equals(entries, javaProject.getRawClasspath()) || !defaultOutput.equals(javaProject.getOutputLocation())) {
            javaProject.setRawClasspath(entries, defaultOutput, progress.split(1));
        }
    }

    /** Links each source root into the project, and drops links that are no longer wanted. */
    private static void linkSourceFolders(IProject project, ImportPlan.Project spec, IProgressMonitor monitor)
        throws CoreException {
        SubMonitor progress = SubMonitor.convert(monitor, spec.sources.size() + 1);
        Map<String, File> links = new HashMap<>();
        for (ImportPlan.SourceRoot source : spec.sources) {
            links.put(source.link, source.dir);
        }
        for (IResource member : project.members()) {
            if (member.isLinked() && !links.containsKey(member.getName())) {
                // Deleting a linked resource removes the link only, never the files it points to.
                member.delete(IResource.NONE, progress.split(0));
            }
        }
        for (Map.Entry<String, File> link : links.entrySet()) {
            IFolder folder = project.getFolder(link.getKey());
            URI target = link.getValue().toURI();
            if (folder.exists() && folder.isLinked() && Objects.equals(sameUri(folder.getLocationURI()), sameUri(target))) {
                continue;
            }
            folder.createLink(target, IResource.REPLACE | IResource.ALLOW_MISSING_LOCAL, progress.split(1));
        }
    }

    private static String sameUri(URI uri) {
        if (uri == null) {
            return null;
        }
        String s = uri.normalize().toString();
        return s.endsWith("/") ? s.substring(0, s.length() - 1) : s;
    }

    private static IPath outputFolder(IProject project, String sourceSet) {
        IFolder bin = project.getFolder(OUTPUT_FOLDER);
        IFolder folder = bin.getFolder(sourceSet);
        try {
            if (!bin.exists()) {
                bin.create(IResource.FORCE | IResource.DERIVED, true, null);
            }
            if (!folder.exists()) {
                folder.create(IResource.FORCE | IResource.DERIVED, true, null);
            }
        } catch (CoreException e) {
            // JDT creates a missing output folder itself on the first build.
        }
        return folder.getFullPath();
    }
}
