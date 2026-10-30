package com.snoutcode.engine.cli;

import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;

class BuildProblemsTest {
    private static List<BuildProblems.Problem> parse(String output) {
        BuildProblems problems = new BuildProblems();
        output.lines().forEach(problems::accept);
        return problems.finish();
    }

    @Test
    void readsJavacErrorsWithCaretAndDetails() {
        List<BuildProblems.Problem> problems = parse("""
            > Task :app:compileJava FAILED
            /src/app/App.java:12: error: cannot find symbol
                    return missing();
                           ^
              symbol:   method missing()
              location: class App
            /src/app/App.java:20: warning: [deprecation] old() in Lib has been deprecated
                    lib.old();
                       ^
            1 error
            1 warning

            FAILURE: Build failed with an exception.
            """);
        assertEquals(2, problems.size(), problems.toString());
        assertEquals(new BuildProblems.Problem("/src/app/App.java", 12, 16, "error",
            "cannot find symbol\nsymbol:   method missing()\nlocation: class App"), problems.get(0));
        assertEquals(new BuildProblems.Problem("/src/app/App.java", 20, 12, "warning",
            "[deprecation] old() in Lib has been deprecated"), problems.get(1));
    }

    @Test
    void readsZincScalaProblems() {
        List<BuildProblems.Problem> problems = parse("""
            > Task :compileScala
            [Error] /src/main/scala/demo/Foo.scala:7:15: type mismatch;
             found   : String("x")
             required: Int
            [Warn] /src/main/scala/demo/Foo.scala:3:1: Unused import
            one error found
            """);
        assertEquals(2, problems.size(), problems.toString());
        assertEquals(new BuildProblems.Problem("/src/main/scala/demo/Foo.scala", 7, 15, "error",
            "type mismatch;\nfound   : String(\"x\")\nrequired: Int"), problems.get(0));
        assertEquals("warning", problems.get(1).severity());
        assertEquals(1, problems.get(1).column());
    }

    @Test
    void readsOlderScalacAndKotlin() {
        List<BuildProblems.Problem> problems = parse("""
            /src/Foo.scala:4: error: not found: value y
              val x = y
                      ^
            e: file:///src/Bar.kt:3:9 Unresolved reference: z
            w: /src/Baz.kt: (8, 2): Parameter 'a' is never used
            """);
        assertEquals(3, problems.size(), problems.toString());
        assertEquals(new BuildProblems.Problem("/src/Foo.scala", 4, 11, "error", "not found: value y"), problems.get(0));
        assertEquals(new BuildProblems.Problem("/src/Bar.kt", 3, 9, "error", "Unresolved reference: z"), problems.get(1));
        assertEquals(new BuildProblems.Problem("/src/Baz.kt", 8, 2, "warning", "Parameter 'a' is never used"), problems.get(2));
    }

    @Test
    void readsMavensCompilerProblems() {
        List<BuildProblems.Problem> problems = parse("""
            [INFO] --- compiler:3.13.0:compile (default-compile) @ app ---
            [ERROR] COMPILATION ERROR :
            [ERROR] /src/app/App.java:[12,16] cannot find symbol
              symbol:   method missing()
              location: class App
            [WARNING] /src/app/Old.java:[3,8] [deprecation] Lib in demo has been deprecated
            [INFO] 1 error
            """);
        assertEquals(2, problems.size(), problems.toString());
        assertEquals(new BuildProblems.Problem("/src/app/App.java", 12, 16, "error",
            "cannot find symbol\nsymbol:   method missing()\nlocation: class App"), problems.get(0));
        assertEquals(new BuildProblems.Problem("/src/app/Old.java", 3, 8, "warning",
            "[deprecation] Lib in demo has been deprecated"), problems.get(1));
    }
}
