package com.snoutcode.engine.cli;

import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

import java.io.File;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.assertEquals;

class GeneratorsTest {
    @Test
    void runsEachBuildsGeneratorsFromItsRoot() {
        JsonObject model = JsonParser.parseString("""
            { "gradleBuilds": [
                { "id": "kafka", "status": "synced", "builds": [
                  { "name": "kafka", "kind": "root", "projects": [
                    { "path": ":clients", "sourceSets": [
                      { "name": "main", "generators": [":clients:processMessages"] },
                      { "name": "test", "generators": [":clients:processTestMessages", ":clients:processMessages"] } ] },
                    { "path": ":core", "sourceSets": [ { "name": "main", "generators": [] } ] } ] },
                  { "name": "tools", "kind": "included", "projects": [
                    { "path": ":", "sourceSets": [ { "name": "main", "generators": [":generateGrammarSource"] } ] } ] },
                  { "name": "buildSrc", "kind": "buildSrc", "projects": [
                    { "path": ":", "sourceSets": [ { "name": "main", "generators": [":gen"] } ] } ] } ] },
                { "id": "quiet", "status": "synced", "builds": [
                  { "name": "quiet", "kind": "root", "projects": [ { "path": ":", "sourceSets": [ { "name": "main" } ] } ] } ] },
                { "id": "broken", "status": "failed", "builds": [] } ],
              "modules": [ { "id": "kafka:clients", "build": "kafka" }, { "id": "kafka:core", "build": "kafka" },
                           { "id": "quiet", "build": "quiet" } ] }""").getAsJsonObject();
        Map<String, ProjectSync.Plan> plans = Map.of(
            "kafka", new ProjectSync.Plan("kafka", new File("/w/kafka"), "9.8.2", null, new Sync.Options(), null),
            "quiet", new ProjectSync.Plan("quiet", new File("/w/quiet"), "9.8.2", null, new Sync.Options(), null),
            "broken", new ProjectSync.Plan("broken", new File("/w/broken"), "9.8.2", null, new Sync.Options(), null));
        List<Generators.Job> jobs = Generators.jobs(model, plans);
        assertEquals(1, jobs.size(), jobs.toString());
        assertEquals(List.of(":clients:processMessages", ":clients:processTestMessages", ":tools:generateGrammarSource"),
            jobs.get(0).tasks());
        assertEquals(List.of("kafka:clients", "kafka:core"), jobs.get(0).modules());
    }

    @Test
    void aMavenBuildGeneratesInTheModulesThatHaveNotYet(@TempDir Path dir) throws Exception {
        Files.createDirectories(dir.resolve("done/target/generated-sources/modello"));
        Files.createDirectories(dir.resolve("model"));
        JsonObject model = JsonParser.parseString("""
            { "gradleBuilds": [
                { "id": "maven", "status": "synced", "tool": "maven", "builds": [
                  { "name": "maven", "kind": "root", "projects": [
                    { "path": ":", "projectDir": "%1$s", "sourceSets": [] },
                    { "path": ":model", "projectDir": "%1$s/model", "sourceSets": [ { "name": "main", "generators": ["process-sources"] } ] },
                    { "path": ":done", "projectDir": "%1$s/done", "sourceSets": [ { "name": "main", "generators": ["process-sources"] } ] },
                    { "path": ":plain", "projectDir": "%1$s/plain", "sourceSets": [ { "name": "main", "generators": [] } ] } ] } ] } ],
              "modules": [ { "id": "maven:model", "build": "maven" } ] }""".formatted(dir.toString().replace("\\", "/")))
            .getAsJsonObject();
        ProjectSync.Plan plan = new ProjectSync.Plan("maven", dir.toFile(), "maven-3.9.16", null, new Sync.Options(), null, true, null);
        List<Generators.Job> jobs = Generators.jobs(model, Map.of("maven", plan));
        assertEquals(1, jobs.size());
        assertEquals(List.of(":model"), jobs.get(0).tasks());
    }
}
