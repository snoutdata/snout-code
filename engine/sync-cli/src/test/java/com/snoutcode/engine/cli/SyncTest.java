package com.snoutcode.engine.cli;

import org.junit.jupiter.api.Test;

import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.IOException;
import java.io.PrintStream;
import java.nio.file.Files;
import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class SyncTest {

    @Test
    void initScriptPathIsStableAndAppliesThePlugin() throws IOException {
        File jar = new File("/opt/it's here/snout-sync-plugin.jar");
        Path first = Sync.writeInitScript(jar);
        Path second = Sync.writeInitScript(jar);
        assertEquals(first, second);
        assertTrue(first.getFileName().toString().startsWith("snout-sync-init"));
        String text = Files.readString(first);
        assertTrue(text.contains("classpath files('" + jar.getAbsolutePath().replace("'", "\\'") + "')"), text);
        assertTrue(text.contains("apply plugin: " + Sync.PLUGIN_CLASS), text);
    }

    @Test
    void rejectsUnknownCommandsAndOptions() {
        ByteArrayOutputStream err = new ByteArrayOutputStream();
        PrintStream stderr = new PrintStream(err);
        assertEquals(2, SnoutEngine.run(new String[] {"build", "."}, System.out, stderr));
        assertEquals(2, SnoutEngine.run(new String[] {"sync", ".", "--bogus"}, System.out, stderr));
        assertEquals(2, SnoutEngine.run(new String[] {"sync", ".", "--out"}, System.out, stderr));
        assertTrue(err.toString().contains("usage: snout-engine sync"));
    }
}
