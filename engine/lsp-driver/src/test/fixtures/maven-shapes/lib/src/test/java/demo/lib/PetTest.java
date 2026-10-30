package demo.lib;

import static org.junit.jupiter.api.Assertions.assertEquals;

import org.junit.jupiter.api.Test;

class PetTest {
    @Test
    void shouts() {
        var pet = new Pet("rex");
        assertEquals("rex", pet.getName());
        assertEquals("REX!", pet.shout());
    }
}
