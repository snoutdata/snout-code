package com.snoutcode.engine.jdtls;

import org.eclipse.core.runtime.CoreException;
import org.eclipse.core.runtime.IPath;
import org.eclipse.core.runtime.IProgressMonitor;
import org.eclipse.core.runtime.IStatus;
import org.eclipse.core.runtime.OperationCanceledException;
import org.eclipse.core.runtime.Status;
import org.eclipse.jdt.ls.core.internal.AbstractProjectImporter;
import org.eclipse.jdt.ls.core.internal.JavaLanguageServerPlugin;

import java.io.File;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Collection;
import java.util.List;

/**
 * Imports a Gradle build into jdtls from the model {@code snout-engine sync} wrote, instead of
 * running Buildship.
 *
 * <p>{@code snout-engine lsp} passes the model file in the {@value #MODEL_PROPERTY} system
 * property. The importer applies to a root folder that is, contains or lies inside the model's
 * root build. It runs before jdtls's Gradle importer and reports every project directory of the
 * model as resolved, so jdtls skips Buildship for them.
 */
public final class SnoutProjectImporter extends AbstractProjectImporter {
    public static final String MODEL_PROPERTY = "snout.model";

    static {
        // jdtls loads the importers at every start: the workspace is saved however it ends.
        SaveOnExit.register();
    }

    private ImportPlan plan;

    @Override
    public boolean applies(IProgressMonitor monitor) throws OperationCanceledException, CoreException {
        if (rootFolder == null) {
            return false;
        }
        if (plan == null) {
            plan = readPlan();
        }
        if (plan == null || !related(rootFolder, plan.rootDir)) {
            return false;
        }
        if (directories == null) {
            List<Path> dirs = new ArrayList<>();
            dirs.add(rootFolder.toPath());
            dirs.add(plan.rootDir.toPath());
            for (File dir : plan.projectDirs) {
                dirs.add(dir.toPath());
            }
            directories = dirs;
        }
        return true;
    }

    @Override
    public boolean applies(Collection<IPath> projectConfigurations, IProgressMonitor monitor)
        throws OperationCanceledException, CoreException {
        return applies(monitor);
    }

    @Override
    public void importToWorkspace(IProgressMonitor monitor) throws OperationCanceledException, CoreException {
        if (!applies(monitor)) {
            return;
        }
        long start = System.currentTimeMillis();
        if (plan.syncError != null) {
            JavaLanguageServerPlugin.logError("SnoutCode: the Gradle sync failed, no projects imported: " + plan.syncError);
        }
        WorkspaceLayout.apply(plan, monitor);
        JavaLanguageServerPlugin.logInfo("SnoutCode: imported " + plan.projects.size() + " project(s) from "
            + plan.rootDir + " in " + (System.currentTimeMillis() - start) + " ms");
    }

    @Override
    public void reset() {
        plan = null;
        directories = null;
    }

    private static ImportPlan readPlan() throws CoreException {
        String property = System.getProperty(MODEL_PROPERTY);
        if (property == null || property.isEmpty()) {
            return null;
        }
        Path file = Path.of(property);
        if (!Files.isRegularFile(file)) {
            JavaLanguageServerPlugin.logInfo("SnoutCode: no model at " + file);
            return null;
        }
        try {
            return ImportPlan.read(file);
        } catch (IOException | RuntimeException e) {
            throw new CoreException(new Status(IStatus.ERROR, SnoutNature.BUNDLE_ID, "Cannot read the build model " + file, e));
        }
    }

    /** Whether a root folder and a Gradle root directory are the same, or one contains the other. */
    static boolean related(File a, File b) {
        Path x = canonical(a);
        Path y = canonical(b);
        return x.startsWith(y) || y.startsWith(x);
    }

    private static Path canonical(File file) {
        try {
            return file.getCanonicalFile().toPath();
        } catch (IOException e) {
            return file.getAbsoluteFile().toPath().normalize();
        }
    }
}
