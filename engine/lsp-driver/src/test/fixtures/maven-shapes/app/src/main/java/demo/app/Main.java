package demo.app;

import demo.lib.Pet;

public class Main {
    public static void main(String[] args) {
        var pet = new Pet("rex");
        System.out.println(pet.getName() + " " + pet.shout());
    }
}
