package com.snoutcode.engine.cli;

import com.google.gson.JsonObject;
import org.junit.jupiter.api.Test;

import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

/** The activity of the engine's Gradle runs, as the app gets it. */
class ActivityTest {
    @Test
    void batchesTheNewestLines() {
        List<JsonObject> sent = new CopyOnWriteArrayList<>();
        Activity.sink(sent::add);
        Activity.say("Download https://repo.example/a.pom");
        Activity.say("  ");
        Activity.say(" Configure project :core ");
        Activity.flush();
        Activity.flush();
        assertEquals(1, sent.size(), "one batch, and nothing when nothing came");
        JsonObject p = sent.get(0).getAsJsonObject("params");
        assertEquals("snout/activity", sent.get(0).get("method").getAsString());
        assertEquals(2, p.getAsJsonArray("lines").size());
        assertEquals("Configure project :core", p.getAsJsonArray("lines").get(1).getAsString());
        assertTrue(!p.has("more"));

        sent.clear();
        for (int i = 0; i < Activity.MAX_LINES + 5; i++) {
            Activity.say("Task :t" + i);
        }
        Activity.flush();
        p = sent.get(0).getAsJsonObject("params");
        assertEquals(Activity.MAX_LINES, p.getAsJsonArray("lines").size());
        assertEquals("Task :t" + (Activity.MAX_LINES + 4), p.getAsJsonArray("lines").get(Activity.MAX_LINES - 1).getAsString());
        assertEquals(5, p.get("more").getAsInt());
    }
}
