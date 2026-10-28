package com.snoutcode.engine.sync.action;

import java.io.Serializable;
import java.util.ArrayList;
import java.util.List;

/** What {@link SyncAction} brings back from the daemon: per build, the JSON of each project. */
public final class SyncResult implements Serializable {
    private static final long serialVersionUID = 1L;

    public String gradleVersion;
    public String gradleJavaHome;
    public final List<Build> builds = new ArrayList<>();

    public static final class Build implements Serializable {
        private static final long serialVersionUID = 1L;

        public String name;
        public String rootDir;
        /** {@code root}, {@code included} or {@code buildSrc}. */
        public String kind;
        public final List<String> projectJson = new ArrayList<>();
        public final List<String> errors = new ArrayList<>();
    }
}
