package com.snoutcode.engine.spring;

import org.junit.jupiter.api.Test;

import java.io.File;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Set;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

class ProfilesTest {
    @Test
    void expressionsTakeNotAndOrAndParentheses() {
        Set<String> active = Set.of("dev", "eu");
        assertTrue(Profiles.evaluate("dev", active));
        assertFalse(Profiles.evaluate("!dev", active));
        assertTrue(Profiles.evaluate("dev & eu", active));
        assertFalse(Profiles.evaluate("dev & us", active));
        assertTrue(Profiles.evaluate("prod | dev", active));
        assertTrue(Profiles.evaluate("cloud | (dev & (eu | us))", active));
        assertFalse(Profiles.evaluate("!(dev & eu)", active));
        assertTrue(Profiles.evaluate("!prod & !test", active));
        // Several values: any one of them.
        assertTrue(Profiles.matches(List.of("prod", "eu"), active));
        assertFalse(Profiles.matches(List.of("prod", "us"), active));
        assertTrue(Profiles.matches(List.of(), active), "no @Profile: always");
        // A typo hides nothing.
        assertTrue(Profiles.evaluate("dev & (", active));
        assertEquals(List.of("cloud", "dev", "eu"), Profiles.names("cloud | !(dev & eu)"));
    }

    @Test
    void theFilesSayWhatIsActiveIncludedGroupedAndDefault() throws Exception {
        Path dir = Files.createTempDirectory("profiles");
        Files.writeString(dir.resolve("application.yml"), """
            spring:
              profiles:
                active: dev
                include:
                  - metrics
                group:
                  dev: h2, debug
            greeting: hello
            ---
            spring:
              config:
                activate:
                  on-profile: prod
            greeting: welcome
            """);
        Files.writeString(dir.resolve("application-dev.properties"), "greeting=hi\n");
        Files.writeString(dir.resolve("application-cloud.yml"), "region: eu\n");
        Profiles.Config c = Profiles.read(List.of(dir.toFile()));
        assertEquals(Set.of("dev", "metrics", "h2", "debug"), Profiles.active(c, null));
        assertEquals(Set.of("prod", "metrics"), Profiles.active(c, List.of("prod")));
        assertEquals(List.of("cloud", "debug", "dev", "h2", "metrics", "prod"), List.copyOf(c.known.keySet()));
        assertEquals("hi", Profiles.values(c, Profiles.active(c, null)).get("greeting"));
        assertEquals("welcome", Profiles.values(c, Profiles.active(c, List.of("prod"))).get("greeting"));
        assertNull(Profiles.values(c, Set.of("prod")).get("region"));
        assertEquals("eu", Profiles.values(c, Set.of("cloud")).get("region"));
        // Nothing active: the default profile, or the ones spring.profiles.default names.
        Path plain = Files.createTempDirectory("profiles-plain");
        Files.writeString(plain.resolve("application.properties"), "spring.profiles.default=local\n#---\nspring.config.activate.on-profile=local\nx=1\n");
        Profiles.Config p = Profiles.read(List.of(plain.toFile()));
        assertEquals(Set.of("local"), Profiles.active(p, null));
        assertEquals("1", Profiles.values(p, Set.of("local")).get("x"));
        assertEquals(Set.of("default"), Profiles.active(new Profiles.Config(), List.of()));
        assertEquals("dev", Profiles.fileProfile("/a/b/application-dev.yml"));
        assertNull(Profiles.fileProfile("application.yml"));
        assertTrue(Profiles.namesProfiles("spring.profiles.active"));
        assertTrue(Profiles.namesProfiles("spring.config.activate.on-profile"));
        assertTrue(Profiles.namesProfiles("spring.profiles.group.dev"));
        for (File f : List.of(dir.toFile(), plain.toFile())) {
            for (File x : f.listFiles()) {
                x.delete();
            }
            f.delete();
        }
    }
}
