package com.snoutcode.engine.cli;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;

import static org.junit.jupiter.api.Assertions.assertEquals;

class TestDiscoveryTest {
    @Test
    void findsJUnit5MethodsAndNestedClasses() {
        String text = """
            package demo.app;

            import org.junit.jupiter.api.Nested;
            import org.junit.jupiter.api.Test;

            /** Not a test: @Test in a comment, "@Test" in a string. */
            class CalcTest {
                private final String s = "void fake() { @Test }";

                @Test
                void adds() {
                    helper(1);
                }

                @org.junit.jupiter.params.ParameterizedTest
                @ValueSource(ints = {1, 2})
                public void many(int n) {
                }

                void helper(int x) {
                    Runnable r = new Runnable() {
                        @Test
                        public void run() {
                        }
                    };
                }

                @Nested
                class Inner {
                    @Test
                    void deep() {
                    }
                }
            }

            abstract class Base {
                @Test
                void inherited() {
                }
            }
            """;
        JsonArray classes = TestDiscovery.find(text, false);
        assertEquals(List.of("demo.app.CalcTest", "demo.app.CalcTest$Inner"), names(classes));
        JsonObject calc = classes.get(0).getAsJsonObject();
        assertEquals(List.of("adds", "many"), methods(calc));
        JsonObject adds = calc.getAsJsonArray("methods").get(0).getAsJsonObject().getAsJsonObject("range");
        assertEquals(10, adds.getAsJsonObject("start").get("line").getAsInt());
        assertEquals(9, adds.getAsJsonObject("start").get("character").getAsInt());
        assertEquals(13, adds.getAsJsonObject("end").get("character").getAsInt());
        JsonObject type = calc.getAsJsonObject("range");
        assertEquals(6, type.getAsJsonObject("start").get("line").getAsInt());
        assertEquals(6, type.getAsJsonObject("start").get("character").getAsInt());
        assertEquals(List.of("deep"), methods(classes.get(1).getAsJsonObject()));
    }

    @Test
    void findsJUnit4AndJUnit3() {
        String text = """
            package old;
            import junit.framework.TestCase;
            public class LegacyTest extends TestCase {
                public void testOne() {}
                public void notATest() {}
                private void testHidden() {}
            }
            class FourTest {
                @org.junit.Test(expected = IllegalStateException.class)
                public void throwsIt() {}
            }
            """;
        JsonArray classes = TestDiscovery.find(text, false);
        assertEquals(List.of("old.LegacyTest", "old.FourTest"), names(classes));
        assertEquals(List.of("testOne"), methods(classes.get(0).getAsJsonObject()));
        assertEquals(List.of("throwsIt"), methods(classes.get(1).getAsJsonObject()));
    }

    @Test
    void findsScalaTestsWithJUnitAnnotations() {
        String text = """
            package com.example
            package core

            import org.junit.jupiter.api.Test

            /* a /* nested */ comment with @Test def no() */
            case class Point(x: Int)

            class CodecTest {
              private val codec = new Codec
              private def roundTrip(s: String): String = s\"\"\"x $s\"\"\"

              @Test
              def survives(): Unit = {
                assert(roundTrip("a") == "x a")
              }

              @Test def `odd name`(): Unit = ()
            }

            object Helpers {
              def main(args: Array[String]): Unit = ()
            }
            """;
        JsonArray classes = TestDiscovery.find(text, true);
        assertEquals(List.of("com.example.core.CodecTest"), names(classes));
        assertEquals(List.of("survives", "odd name"), methods(classes.get(0).getAsJsonObject()));
    }

    private static List<String> names(JsonArray classes) {
        List<String> out = new ArrayList<>();
        classes.forEach(c -> out.add(c.getAsJsonObject().get("name").getAsString()));
        return out;
    }

    private static List<String> methods(JsonObject type) {
        List<String> out = new ArrayList<>();
        type.getAsJsonArray("methods").forEach(m -> out.add(m.getAsJsonObject().get("name").getAsString()));
        return out;
    }
}
