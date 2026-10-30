package demo.core;

import static org.junit.jupiter.api.Assertions.assertEquals;

import org.junit.jupiter.api.Test;

public class GreeterTest {
    public static final String WHO = "maven";

    @Test
    void greets() {
        assertEquals("Hello, Maven!", new Greeter().greet(WHO));
    }
}
