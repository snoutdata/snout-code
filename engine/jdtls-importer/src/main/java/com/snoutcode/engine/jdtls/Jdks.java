package com.snoutcode.engine.jdtls;

import org.eclipse.core.runtime.CoreException;
import org.eclipse.jdt.core.IClasspathEntry;
import org.eclipse.jdt.core.IJavaProject;
import org.eclipse.jdt.core.JavaCore;
import org.eclipse.jdt.launching.IVMInstall;
import org.eclipse.jdt.launching.IVMInstallType;
import org.eclipse.jdt.launching.JavaRuntime;
import org.eclipse.jdt.launching.VMStandin;
import org.eclipse.jdt.launching.environments.IExecutionEnvironment;
import org.eclipse.jdt.ls.core.internal.JavaLanguageServerPlugin;

import java.io.File;
import java.io.IOException;
import java.util.HashSet;
import java.util.Map;
import java.util.Set;

/** The JDK and compiler settings of an imported project. */
final class Jdks {
    private static final String STANDARD_VM_TYPE = "org.eclipse.jdt.internal.debug.ui.launcher.StandardVMType";
    private static final String MAC_VM_TYPE = "org.eclipse.jdt.internal.launching.macosx.MacOSXType";

    /** Resource types that are not Java and should not be copied to the output folders. */
    private static final String RESOURCE_COPY_FILTER = "*.launch,*.scala,*.kt,*.groovy,*.gradle,*.kts";

    private Jdks() {
    }

    /**
     * The JRE container for the project's JDK: the toolchain's or the Gradle daemon's Java home,
     * registered as a VM install if jdtls does not know it yet, else the execution environment
     * for the release, else the default VM.
     */
    static IClasspathEntry containerEntry(ImportPlan.JavaSettings java) {
        IVMInstall vm = java.javaHome == null ? null : findOrAdd(new File(java.javaHome));
        if (vm != null) {
            return JavaCore.newContainerEntry(JavaRuntime.newJREContainerPath(vm));
        }
        IExecutionEnvironment environment = environment(level(java));
        if (environment != null) {
            return JavaCore.newContainerEntry(JavaRuntime.newJREContainerPath(environment));
        }
        return JavaRuntime.getDefaultJREContainerEntry();
    }

    static void applyCompilerOptions(IJavaProject project, ImportPlan.JavaSettings java) {
        Map<String, String> options = project.getOptions(false);
        String level = level(java);
        if (level != null) {
            JavaCore.setComplianceOptions(level, options);
            if (java.target != null && java.release == null) {
                options.put(JavaCore.COMPILER_CODEGEN_TARGET_PLATFORM, normalizeLevel(java.target));
            }
            options.put(JavaCore.COMPILER_RELEASE, java.release != null ? JavaCore.ENABLED : JavaCore.DISABLED);
        }
        options.put(JavaCore.COMPILER_CODEGEN_METHOD_PARAMETERS_ATTR,
            java.parameters ? JavaCore.GENERATE : JavaCore.DO_NOT_GENERATE);
        options.put(JavaCore.COMPILER_PB_ENABLE_PREVIEW_FEATURES, java.preview ? JavaCore.ENABLED : JavaCore.DISABLED);
        if (java.preview) {
            options.put(JavaCore.COMPILER_PB_REPORT_PREVIEW_FEATURES, JavaCore.IGNORE);
        }
        options.put(JavaCore.CORE_JAVA_BUILD_RESOURCE_COPY_FILTER, RESOURCE_COPY_FILTER);
        if (!options.equals(project.getOptions(false))) {
            project.setOptions(options);
        }
    }

    private static String level(ImportPlan.JavaSettings java) {
        String level = java.release != null ? java.release : java.source;
        return level == null ? null : normalizeLevel(level);
    }

    /** Gradle's {@code 1.8} and {@code 8}, {@code 17} and {@code VERSION_17} to JDT's levels. */
    static String normalizeLevel(String level) {
        String s = level.trim().replace("VERSION_", "").replace('_', '.');
        if (s.matches("[5-8]")) {
            s = "1." + s;
        }
        return s;
    }

    private static IExecutionEnvironment environment(String level) {
        if (level == null) {
            return null;
        }
        String name = level.startsWith("1.") && !level.equals("1.8") ? "J2SE-" + level : "JavaSE-" + level;
        return JavaRuntime.getExecutionEnvironmentsManager().getEnvironment(name);
    }

    /** The VM install at this Java home, added to jdtls's list when it is not there yet. */
    private static synchronized IVMInstall findOrAdd(File javaHome) {
        File home = canonical(javaHome);
        if (!new File(home, "bin").isDirectory()) {
            return null;
        }
        Set<String> names = new HashSet<>();
        for (IVMInstallType type : JavaRuntime.getVMInstallTypes()) {
            for (IVMInstall vm : type.getVMInstalls()) {
                names.add(vm.getName());
                if (vm.getInstallLocation() != null && canonical(vm.getInstallLocation()).equals(home)) {
                    return vm;
                }
            }
        }
        IVMInstallType type = JavaRuntime.getVMInstallType(STANDARD_VM_TYPE);
        if (type == null || !type.validateInstallLocation(home).isOK()) {
            type = JavaRuntime.getVMInstallType(MAC_VM_TYPE);
        }
        if (type == null || !type.validateInstallLocation(home).isOK()) {
            return null;
        }
        String id = "snout-" + Integer.toHexString(home.getPath().hashCode());
        for (int i = 2; type.findVMInstall(id) != null; i++) {
            id = "snout-" + Integer.toHexString(home.getPath().hashCode()) + "-" + i;
        }
        String base = home.getName().equals("Home") && home.getParentFile() != null && home.getParentFile().getParentFile() != null
            ? home.getParentFile().getParentFile().getName() // .../jdk-17.jdk/Contents/Home
            : home.getName();
        String name = base;
        for (int i = 2; names.contains(name); i++) {
            name = base + " (" + i + ")";
        }
        VMStandin standin = new VMStandin(type, id);
        standin.setName(name);
        standin.setInstallLocation(home);
        IVMInstall vm = standin.convertToRealVM();
        try {
            JavaRuntime.saveVMConfiguration();
        } catch (CoreException e) {
            JavaLanguageServerPlugin.logException("SnoutCode: could not save the VM configuration", e);
        }
        JavaLanguageServerPlugin.logInfo("SnoutCode: added JDK " + name + " at " + home);
        return vm;
    }

    private static File canonical(File file) {
        try {
            return file.getCanonicalFile();
        } catch (IOException e) {
            return file.getAbsoluteFile();
        }
    }
}
