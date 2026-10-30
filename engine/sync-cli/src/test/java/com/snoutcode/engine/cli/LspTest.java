package com.snoutcode.engine.cli;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.io.File;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

class LspTest {
    @TempDir
    Path dir;

    @Test
    void lombokRunsInJdtlsWhenAProjectUsesIt() throws IOException {
        JsonObject model = new JsonObject();
        JsonArray libraries = new JsonArray();
        libraries.add(library("org.slf4j", "slf4j-api", "2.0.18"));
        model.add("libraries", libraries);
        assertNull(Lsp.lombokAgent(model));

        libraries.add(library("org.projectlombok", "lombok", "1.18.9"));
        libraries.add(library("org.projectlombok", "lombok", "1.18.38"));
        libraries.add(library("org.projectlombok", "lombok", "1.18.10"));
        File agent = Lsp.lombokAgent(model);
        assertEquals("lombok-1.18.38.jar", agent.getName(), "the newest");
        assertTrue(Lsp.compareVersions("1.18.10", "1.18.9") > 0);
    }

    private JsonObject library(String group, String name, String version) throws IOException {
        Path file = dir.resolve(name + "-" + version + ".jar");
        Files.write(file, new byte[0]);
        JsonObject lib = new JsonObject();
        lib.addProperty("id", group + ":" + name + ":" + version);
        lib.addProperty("group", group);
        lib.addProperty("name", name);
        lib.addProperty("version", version);
        lib.addProperty("file", file.toString());
        return lib;
    }
}
