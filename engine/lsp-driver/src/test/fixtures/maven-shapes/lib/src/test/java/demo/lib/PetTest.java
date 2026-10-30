package demo.lib;

import static org.junit.jupiter.api.Assertions.assertEquals;

import java.util.List;
import org.apache.commons.collections4.CollectionUtils;
import org.junit.jupiter.api.Test;

class PetTest {
    @Test
    void shouts() {
        var pet = new Pet("rex");
        assertEquals("rex", pet.getName());
        assertEquals("REX!", pet.shout());
        assertEquals(1, CollectionUtils.size(List.of(pet)));
    }
}
