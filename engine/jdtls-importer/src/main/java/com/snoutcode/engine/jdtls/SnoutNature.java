package com.snoutcode.engine.jdtls;

import org.eclipse.core.resources.IProject;
import org.eclipse.core.resources.IProjectNature;
import org.eclipse.core.runtime.CoreException;

/** Marks the JDT projects this bundle imported, so its build support owns them. */
public final class SnoutNature implements IProjectNature {
    static final String BUNDLE_ID = "com.snoutcode.engine.jdtls";
    public static final String ID = BUNDLE_ID + ".nature";

    private IProject project;

    static boolean has(IProject project) {
        try {
            return project != null && project.isAccessible() && project.hasNature(ID);
        } catch (CoreException e) {
            return false;
        }
    }

    @Override
    public void configure() {
    }

    @Override
    public void deconfigure() {
    }

    @Override
    public IProject getProject() {
        return project;
    }

    @Override
    public void setProject(IProject project) {
        this.project = project;
    }
}
