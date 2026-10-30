package com.snoutcode.engine.jdtls;

import org.eclipse.core.resources.IFolder;
import org.eclipse.core.resources.IResource;
import org.eclipse.core.resources.IWorkspace;
import org.eclipse.core.resources.ResourcesPlugin;
import org.eclipse.core.runtime.IProgressMonitor;
import org.eclipse.core.runtime.Path;
import org.eclipse.core.runtime.jobs.Job;
import org.eclipse.jdt.core.JavaCore;
import org.eclipse.jdt.internal.core.ExternalFoldersManager;
import org.eclipse.jdt.ls.core.internal.IDelegateCommandHandler;
import org.eclipse.jdt.ls.core.internal.JavaLanguageServerPlugin;

import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.lang.reflect.Constructor;
import java.lang.reflect.Method;
import java.lang.management.LockInfo;
import java.lang.management.ManagementFactory;
import java.lang.management.MonitorInfo;
import java.lang.management.ThreadInfo;
import java.net.InetAddress;
import java.net.ServerSocket;
import java.net.Socket;
import java.util.List;

import org.eclipse.core.runtime.Platform;
import org.osgi.framework.Bundle;

/**
 * Commands snout-engine sends jdtls through {@code workspace/executeCommand}.
 *
 * <p>{@code snout.refreshClassFolders [folder...]}: after the engine recompiles Scala, refreshes
 * the class folders it compiled into. JDT holds a folder outside the workspace as a link in its
 * hidden external folders project and reads it again only when that link is refreshed; the
 * resource change then rebuilds the projects that use the folder. Answers how many folders it
 * refreshed.
 *
 * <p>{@code snout.reimport}: reads the model file again and brings the JDT projects in line with
 * it, as the import at the start does, after the engine synced a build again or the Scala bridge
 * changed its class folders. Answers how many projects the model has.
 *
 * <p>{@code snout.threadDump}: jdtls's threads, their stacks and the locks they hold or wait
 * for, as text: what the engine logs when a command to jdtls times out.
 *
 * <p>{@code snout.startDebugSession}: java-debug's {@code vscode.java.startDebugSession}, on the
 * loopback address. java-debug's own server listens on every address, so Windows Firewall asks
 * the user whether Java may take connections from the network each time a debug session starts;
 * nothing but the app on this machine connects to it. Answers the port, which takes one
 * connection, the DAP session java-debug serves on it.
 */
public final class SnoutCommands implements IDelegateCommandHandler {
    @Override
    public Object executeCommand(String commandId, List<Object> arguments, IProgressMonitor monitor) throws Exception {
        if ("snout.reimport".equals(commandId)) {
            return reimport(monitor);
        }
        if ("snout.startDebugSession".equals(commandId)) {
            return startDebugSession();
        }
        if ("snout.threadDump".equals(commandId)) {
            return threadDump();
        }
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

    private static int reimport(IProgressMonitor monitor) throws Exception {
        long start = System.currentTimeMillis();
        String property = System.getProperty(SnoutProjectImporter.MODEL_PROPERTY);
        if (property == null || property.isEmpty()) {
            return 0;
        }
        ImportPlan plan = ImportPlan.read(java.nio.file.Path.of(property));
        IWorkspace workspace = ResourcesPlugin.getWorkspace();
        // The import holds the whole workspace, so it waits for a running build, which on a large
        // project (the first, full one) takes minutes. Its classpath changes start a new build
        // anyway: stop this one rather than wait for it.
        Job.getJobManager().cancel(ResourcesPlugin.FAMILY_AUTO_BUILD);
        workspace.run(m -> WorkspaceLayout.apply(plan, m), workspace.getRoot(), IWorkspace.AVOID_UPDATE, monitor);
        JavaLanguageServerPlugin.logInfo("SnoutCode: imported " + plan.projects.size() + " project(s) again in "
            + (System.currentTimeMillis() - start) + " ms");
        return plan.projects.size();
    }

    /** Every thread with its state, the lock it waits for and who holds it, and its stack. */
    static String threadDump() {
        StringBuilder out = new StringBuilder();
        for (ThreadInfo t : ManagementFactory.getThreadMXBean().dumpAllThreads(true, true)) {
            out.append('"').append(t.getThreadName()).append("\" ").append(t.getThreadState());
            if (t.getLockName() != null) {
                out.append(" on ").append(t.getLockName());
            }
            if (t.getLockOwnerName() != null) {
                out.append(" held by \"").append(t.getLockOwnerName()).append('"');
            }
            out.append('\n');
            StackTraceElement[] stack = t.getStackTrace();
            for (int i = 0; i < Math.min(stack.length, 40); i++) {
                out.append("    at ").append(stack[i]).append('\n');
            }
            for (MonitorInfo m : t.getLockedMonitors()) {
                out.append("    holds ").append(m).append('\n');
            }
            for (LockInfo l : t.getLockedSynchronizers()) {
                out.append("    holds ").append(l).append('\n');
            }
        }
        // The workspace's lock is a job rule, no Java monitor: the running jobs say who has it.
        out.append("Running jobs:\n");
        for (Job job : Job.getJobManager().find(null)) {
            if (job.getState() == Job.RUNNING) {
                out.append("    ").append(job.getName()).append(" (").append(job.getClass().getName()).append(", rule ")
                    .append(job.getRule()).append(", thread ").append(job.getThread() == null ? "?" : job.getThread().getName()).append(")\n");
            }
        }
        return out.toString();
    }

    /** How long a debug session's port waits for the app: a Gradle task compiles first. */
    private static final int DEBUG_ACCEPT_MILLIS = 30 * 60 * 1000;

    private static int startDebugSession() throws Exception {
        // java-debug exports none of its packages: its classes come from its bundle's own loader.
        Bundle bundle = Platform.getBundle("com.microsoft.java.debug.plugin");
        if (bundle == null) {
            throw new IllegalStateException("java-debug is not loaded");
        }
        Class<?> contextClass = bundle.loadClass("com.microsoft.java.debug.core.adapter.IProviderContext");
        Method context = bundle.loadClass("com.microsoft.java.debug.plugin.internal.JdtProviderContextFactory")
            .getMethod("createProviderContext");
        Constructor<?> server = bundle.loadClass("com.microsoft.java.debug.core.adapter.ProtocolServer")
            .getConstructor(InputStream.class, OutputStream.class, contextClass);
        Method run = server.getDeclaringClass().getMethod("run");
        ServerSocket socket = new ServerSocket(0, 1, InetAddress.getLoopbackAddress());
        socket.setSoTimeout(DEBUG_ACCEPT_MILLIS);
        Thread serve = new Thread(() -> {
            try (ServerSocket s = socket; Socket connection = s.accept()) {
                run.invoke(server.newInstance(connection.getInputStream(), connection.getOutputStream(),
                    context.invoke(null)));
            } catch (IOException e) {
                JavaLanguageServerPlugin.logInfo("SnoutCode: debug session ended: " + e);
            } catch (ReflectiveOperationException e) {
                JavaLanguageServerPlugin.logException("SnoutCode: debug session failed", e);
            }
        }, "snout-debug-session");
        serve.setDaemon(true);
        serve.start();
        JavaLanguageServerPlugin.logInfo("SnoutCode: debug session on " + socket.getLocalSocketAddress());
        return socket.getLocalPort();
    }
}
