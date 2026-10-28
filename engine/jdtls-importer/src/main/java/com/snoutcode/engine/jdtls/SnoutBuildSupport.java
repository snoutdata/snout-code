package com.snoutcode.engine.jdtls;

import org.eclipse.core.resources.IProject;
import org.eclipse.core.resources.IResource;
import org.eclipse.core.runtime.CoreException;
import org.eclipse.core.runtime.IPath;
import org.eclipse.core.runtime.IProgressMonitor;
import org.eclipse.jdt.ls.core.internal.JavaLanguageServerPlugin;
import org.eclipse.jdt.ls.core.internal.managers.IBuildSupport;

import java.io.File;
import java.util.ArrayList;
import java.util.Collection;

/**
 * Build support for the projects {@link SnoutProjectImporter} creates.
 *
 * <p>Those projects live in the workspace data directory, outside every root folder, so jdtls's
 * own cleanup would delete them on each start as strays. This class claims them and deletes only
 * the ones whose Gradle root is no longer open.
 *
 * <p>The build model comes from {@code snout-engine sync}, which runs before jdtls starts, so
 * there is nothing for jdtls to update here.
 */
public final class SnoutBuildSupport implements IBuildSupport {
    @Override
    public boolean applies(IProject project) {
        return SnoutNature.has(project);
    }

    @Override
    public boolean isBuildFile(IResource resource) {
        return false;
    }

    @Override
    public void update(IProject project, boolean force, IProgressMonitor monitor) throws CoreException {
        // The model is refreshed by snout-engine, which restarts jdtls with the new one.
    }

    @Override
    public boolean hasSpecificDeleteProjectLogic() {
        return true;
    }

    @Override
    public void deleteInvalidProjects(Collection<IPath> rootPaths, ArrayList<IProject> candidates, IProgressMonitor monitor) {
        for (IProject project : candidates) {
            if (!applies(project)) {
                continue;
            }
            try {
                String root = project.getPersistentProperty(WorkspaceLayout.ROOT_DIR);
                if (root == null || rootPaths.stream().noneMatch(p -> SnoutProjectImporter.related(p.toFile(), new File(root)))) {
                    project.delete(false, true, monitor);
                }
            } catch (CoreException e) {
                JavaLanguageServerPlugin.logException("SnoutCode: could not remove project " + project.getName(), e);
            }
        }
    }

    @Override
    public String buildToolName() {
        return "Gradle";
    }

    @Override
    public String unsupportedOperationMessage() {
        return "SnoutCode reads the source folders and classpath from the Gradle build. Change the build file instead.";
    }
}
