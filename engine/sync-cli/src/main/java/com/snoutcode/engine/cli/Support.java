package com.snoutcode.engine.cli;

import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * The Gradle and Maven versions SnoutCode supports, tested on JDKs 17, 21 and 25 (see
 * docs/supported-versions.md): Gradle 7.0 to 9.x, and Maven 3.6 to 3.9 through the build's
 * wrapper. A build outside the range still syncs if it can; the model says why it may not work
 * ({@code unsupportedBuilds}), and the app shows it.
 */
final class Support {
    static final String GRADLE_RANGE = "Gradle 7.0 to 9.x";
    static final String MAVEN_RANGE = "Maven 3.6 to 3.9";

    private static final Pattern VERSION = Pattern.compile("^(\\d+)(?:\\.(\\d+))?");

    private Support() {
    }

    /** Why a Gradle version is outside the range, for people; null when it is in it (or unknown). */
    static String gradle(String version) {
        int[] v = parse(version);
        if (v == null) {
            return null;
        }
        if (v[0] < 7) {
            return "Gradle " + version + " is older than SnoutCode supports (" + GRADLE_RANGE + "): it may sync, but parts of "
                + "the build can be missing or wrong. Update the build's wrapper to Gradle 7 or newer.";
        }
        if (v[0] > 9) {
            return "Gradle " + version + " is newer than SnoutCode supports (" + GRADLE_RANGE + ") and was not tested with it.";
        }
        return null;
    }

    /** Why a Maven version (the build's wrapper's) is outside the range; null when it is in it. */
    static String maven(String version) {
        int[] v = parse(version);
        if (v == null) {
            return null;
        }
        if (v[0] < 3 || v[0] == 3 && v[1] < 6) {
            return "Maven " + version + " is older than SnoutCode supports (" + MAVEN_RANGE + "): goals run with it, but the "
                + "build is read as Maven 3.9 reads it. Update the build's wrapper to Maven 3.6 or newer.";
        }
        if (v[0] > 3) {
            return "Maven " + version + " is not supported yet (" + MAVEN_RANGE + "): SnoutCode reads the build as Maven 3.9 "
                + "does, which a Maven 4 build may not allow.";
        }
        return null;
    }

    private static int[] parse(String version) {
        if (version == null) {
            return null;
        }
        Matcher m = VERSION.matcher(version.trim());
        return m.find() ? new int[] {Integer.parseInt(m.group(1)), m.group(2) == null ? 0 : Integer.parseInt(m.group(2))} : null;
    }
}
