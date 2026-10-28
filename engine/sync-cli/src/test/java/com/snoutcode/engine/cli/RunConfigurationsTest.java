package com.snoutcode.engine.cli;

import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;

class RunConfigurationsTest {
    @Test
    void findsJavaMains() {
        assertEquals(List.of("a.b.App"), RunConfigurations.mains("App.java",
            "package a.b;\npublic class App {\n  public static void main(final String... args) {}\n}\n"));
        assertEquals(List.of("App"), RunConfigurations.mains("App.java", "class App { static public void main(String[] a) {} }"));
        assertEquals(List.of(), RunConfigurations.mains("App.java", "class App { void main(String[] a) {} }"));
    }

    @Test
    void findsScalaMains() {
        String text = """
            package tools

            object Tool {
              def main(args: Array[String]): Unit = println("x")
            }

            object Quick extends App {
              println("y")
            }

            class NotMain { def main(): Unit = () }
            """;
        assertEquals(List.of("tools.Tool", "tools.Quick"), RunConfigurations.mains("Tool.scala", text));
    }

    @Test
    void findsTheProjectOfAClasspath() {
        assertEquals("app", Launches.projectInWorkspace(java.nio.file.Path.of("/c/workspace"),
            List.of("/c/build/x/classes", "/c/workspace/app/bin/main", "/c/workspace/lib/bin/main")));
        assertEquals(null, Launches.projectInWorkspace(java.nio.file.Path.of("/c/workspace"), List.of("/m/a.jar")));
    }

    @Test
    void methodNamesDropParametersAndIndexes() {
        assertEquals("adds", GradleRuns.methodName("adds()"));
        assertEquals("many", GradleRuns.methodName("many(int)[2]"));
        assertEquals("plain", GradleRuns.methodName("plain"));
    }

    @Test
    void testFilters() throws Exception {
        assertEquals("a.b.*", Engine.testFilter(EngineIntegrationTest.parse("{\"kind\":\"package\",\"package\":\"a.b\"}")));
        assertEquals("a.B", Engine.testFilter(EngineIntegrationTest.parse("{\"kind\":\"class\",\"class\":\"a.B\"}")));
        assertEquals("a.B.m", Engine.testFilter(EngineIntegrationTest.parse("{\"kind\":\"method\",\"class\":\"a.B\",\"method\":\"m\"}")));
        assertEquals(null, Engine.testFilter(EngineIntegrationTest.parse("{\"kind\":\"all\"}")));
    }
}
