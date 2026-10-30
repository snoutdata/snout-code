package demo.app;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

import demo.core.GreeterTest;
import java.nio.file.Files;
import java.nio.file.Path;
import org.junit.jupiter.api.Test;

class MainTest {
    @Test
    void adds() {
        assertEquals(5, Main.add(2, 3));
    }

    @Test
    void usesTheLibrarysTests() {
        assertEquals("maven", GreeterTest.WHO);
    }

    /** Fails until the file it looks for exists: for rerunning failed tests. */
    @Test
    void passesOnceFixed() {
        assertTrue(Files.exists(Path.of("fixed.txt")), "not fixed yet");
    }
}
