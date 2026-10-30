package com.snoutcode.engine.cli;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import org.junit.jupiter.api.Test;

class SupportTest {
    @Test
    void gradleSevenToNineIsSupported() {
        for (String v : new String[] {"7.0", "7.0.2", "7.6.6", "8.14.3", "9.0.0", "9.8.2"}) {
            assertNull(Support.gradle(v), v);
        }
        assertTrue(Support.gradle("6.9.4").startsWith("Gradle 6.9.4 is older than SnoutCode supports (Gradle 7.0 to 9.x)"));
        assertTrue(Support.gradle("10.0").startsWith("Gradle 10.0 is newer than SnoutCode supports"));
        assertNull(Support.gradle(null), "no wrapper: the Tooling API's own Gradle");
    }

    @Test
    void mavenThreeSixToThreeNineIsSupported() {
        for (String v : new String[] {"3.6.3", "3.8.8", "3.9.16"}) {
            assertNull(Support.maven(v), v);
        }
        assertTrue(Support.maven("3.5.4").startsWith("Maven 3.5.4 is older than SnoutCode supports (Maven 3.6 to 3.9)"));
        assertTrue(Support.maven("4.0.0-rc-5").startsWith("Maven 4.0.0-rc-5 is not supported yet"));
    }

    @Test
    void theModelListsTheBuildsOutsideTheRange() {
        JsonObject model = new JsonObject();
        model.addProperty("rootDir", "/w");
        JsonArray builds = new JsonArray();
        for (String[] b : new String[][] {{"old", "6.0.1"}, {"new", "8.14.3"}}) {
            JsonObject e = new JsonObject();
            e.addProperty("id", b[0]);
            e.addProperty("rootDir", "/w/" + b[0]);
            e.addProperty("gradleVersion", b[1]);
            e.addProperty("status", "synced");
            e.add("builds", new JsonArray());
            builds.add(e);
        }
        model.add("gradleBuilds", builds);
        ProjectModel.assemble(model);
        JsonArray unsupported = model.getAsJsonArray("unsupportedBuilds");
        assertEquals(1, unsupported.size());
        assertEquals("old", unsupported.get(0).getAsJsonObject().get("build").getAsString());
        assertEquals("gradle", unsupported.get(0).getAsJsonObject().get("tool").getAsString());
    }
}
