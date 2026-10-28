package com.snoutcode.engine.jdtls;

import org.eclipse.core.resources.IFolder;
import org.eclipse.core.resources.IResource;
import org.eclipse.core.resources.ResourcesPlugin;
import org.eclipse.core.runtime.IProgressMonitor;
import org.eclipse.core.runtime.Path;
import org.eclipse.jdt.core.JavaCore;
import org.eclipse.jdt.internal.core.ExternalFoldersManager;
import org.eclipse.jdt.ls.core.internal.IDelegateCommandHandler;
import org.eclipse.jdt.ls.core.internal.JavaLanguageServerPlugin;

import java.util.List;

/**
 * Commands snout-engine sends jdtls through {@code workspace/executeCommand}.
 *
 * <p>{@code snout.refreshClassFolders [folder...]}: after the engine recompiles Scala, refreshes
 * the class folders it compiled into. JDT holds a folder outside the workspace as a link in its
 * hidden external folders project and reads it again only when that link is refreshed; the
 * resource change then rebuilds the projects that use the folder. Answers how many folders it
 * refreshed.
 */
public final class SnoutCommands implements IDelegateCommandHandler {
    @Override
    public Object executeCommand(String commandId, List<Object> arguments, IProgressMonitor monitor) throws Exception {
        if (!"snout.refreshClassFolders".equals(commandId)) {
            throw new UnsupportedOperationException(commandId);
        }
        long start = System.currentTimeMillis();
        int refreshed = 0;
        ExternalFoldersManager folders = ExternalFoldersManager.getExternalFoldersManager();
        for (Object argument : arguments == null ? List.of() : arguments) {
            IFolder link = folders.getFolder(new Path(String.valueOf(argument)));
            if (link != null && link.exists()) {
                link.refreshLocal(IResource.DEPTH_INFINITE, monitor);
                refreshed++;
            }
        }
        if (refreshed == 0) {
            // Not linked yet (a folder that was empty at import): let JDT look at every library again.
            JavaCore.create(ResourcesPlugin.getWorkspace().getRoot()).refreshExternalArchives(null, monitor);
        }
        JavaLanguageServerPlugin.logInfo("SnoutCode: refreshed " + refreshed + " class folder(s) in "
            + (System.currentTimeMillis() - start) + " ms");
        return refreshed;
    }
}
