package demo.app;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;

class GreeterTest {
    @Test
    void greets() {
        assertEquals("hello you", Greeter.greet("you"));
    }
}
