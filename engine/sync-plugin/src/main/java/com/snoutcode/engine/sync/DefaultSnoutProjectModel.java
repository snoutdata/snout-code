package com.snoutcode.engine.sync;

import com.snoutcode.engine.sync.model.SnoutProjectModel;

import java.io.Serializable;

/** Daemon-side implementation of {@link SnoutProjectModel}; serialized to the client. */
public final class DefaultSnoutProjectModel implements SnoutProjectModel, Serializable {
    private static final long serialVersionUID = 1L;

    private final String projectPath;
    private final String json;

    public DefaultSnoutProjectModel(String projectPath, String json) {
        this.projectPath = projectPath;
        this.json = json;
    }

    @Override
    public String getProjectPath() {
        return projectPath;
    }

    @Override
    public String getJson() {
        return json;
    }
}
