// Copyright 2000-2023 JetBrains s.r.o. and contributors. Use of this source code is governed by the Apache 2.0 license.
// Adapted for SnoutCode from IntelliJ IDEA Community Edition,
// plugins/gradle/tooling-extension-impl/resources/org/jetbrains/plugins/gradle/tooling/internal/init/JetGradlePlugin.gradle.
// Changes: ported from Groovy to Java, registers SnoutCode's single project model builder, and
// installs the script tracker that records `apply from` scripts.
package com.snoutcode.engine.sync;

import com.snoutcode.engine.sync.model.SnoutProjectModel;
import org.gradle.api.Plugin;
import org.gradle.api.invocation.Gradle;
import org.gradle.tooling.provider.model.ToolingModelBuilderRegistry;
import org.gradle.tooling.provider.model.UnknownModelException;

import javax.inject.Inject;

/**
 * Applied to every build of the composite by the init script that {@code snout-engine sync}
 * generates. It registers the model builder for {@link SnoutProjectModel} with the build's
 * tooling registry and starts recording which scripts each project applies.
 */
public class SnoutSyncPlugin implements Plugin<Gradle> {
    private final ToolingModelBuilderRegistry registry;

    @Inject
    public SnoutSyncPlugin(ToolingModelBuilderRegistry registry) {
        this.registry = registry;
    }

    @Override
    public void apply(Gradle gradle) {
        ScriptTracker.install(gradle);
        if (!isRegistered(registry)) {
            registry.register(new ProjectModelBuilder());
        }
    }

    private static boolean isRegistered(ToolingModelBuilderRegistry registry) {
        try {
            registry.getBuilder(SnoutProjectModel.class.getName());
            return true;
        } catch (UnknownModelException e) {
            return false;
        }
    }
}
