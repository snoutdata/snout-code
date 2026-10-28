package com.snoutcode.engine.sync.model;

/**
 * The model of one Gradle project, as the Tooling API client sees it.
 *
 * <p>The daemon-side builder returns an object with the same getters; the Tooling API adapts it
 * to this interface across the class loader boundary. The payload is a JSON object, which keeps
 * the wire contract down to one string and lets the model grow without new interfaces.
 */
public interface SnoutProjectModel {

    /** The project's Gradle path, for example {@code :} or {@code :app}. */
    String getProjectPath();

    /** The project model as a JSON object. */
    String getJson();
}
