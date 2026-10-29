package demo;

import static org.junit.jupiter.api.Assertions.assertEquals;

import org.junit.jupiter.api.Test;

class GreeterTest {
    @Test
    void greets() {
        assertEquals("Hello, test", Greeter.greet("test"));
    }

    @Test
    void adds() {
        assertEquals(5, Greeter.add(2, 3));
    }
}
