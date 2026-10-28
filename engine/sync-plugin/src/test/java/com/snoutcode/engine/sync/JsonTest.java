package com.snoutcode.engine.sync;

import org.junit.jupiter.api.Test;

import java.io.File;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.Map;

import static org.junit.jupiter.api.Assertions.assertEquals;

class JsonTest {

    @Test
    void writesNestedValues() {
        Map<String, Object> map = new LinkedHashMap<>();
        map.put("s", "a");
        map.put("n", 3);
        map.put("b", true);
        map.put("z", null);
        map.put("l", Arrays.asList(1, "x", null));
        map.put("m", new LinkedHashMap<>());
        assertEquals("{\"s\":\"a\",\"n\":3,\"b\":true,\"z\":null,\"l\":[1,\"x\",null],\"m\":{}}", Json.write(map));
    }

    @Test
    void escapesStrings() {
        assertEquals("\"q\\\" b\\\\ n\\n t\\t c\\u0001\"", Json.write("q\" b\\ n\n t\t c\u0001"));
    }

    @Test
    void writesFilesAsAbsolutePaths() {
        File f = new File("/tmp/x y.jar");
        assertEquals("\"" + f.getAbsolutePath() + "\"", Json.write(f));
    }

    @Test
    void writesNonFiniteNumbersAsNull() {
        assertEquals("[null,1.5]", Json.write(Arrays.asList(Double.NaN, 1.5)));
    }
}
