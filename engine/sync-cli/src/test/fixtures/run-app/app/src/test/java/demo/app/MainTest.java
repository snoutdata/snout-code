package demo.app;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;

class MainTest {
    @Test
    void adds() {
        System.out.println("adding in a test");
        assertEquals(3, Main.add(1, 2));
    }

    @Test
    void failsOnPurpose() {
        assertEquals(5, Main.add(2, 2), "two and two");
    }
}
