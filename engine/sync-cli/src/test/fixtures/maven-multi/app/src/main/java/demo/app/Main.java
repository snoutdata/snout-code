package demo.app;

import demo.core.Greeter;

import java.io.InputStream;
import java.nio.charset.StandardCharsets;

public class Main {
    public static void main(String[] args) throws Exception {
        int total = 0;
        for (int i = 1; i <= 3; i++) {
            total = add(total, i);
        }
        System.out.println("total " + total);
        System.out.println(new Greeter().greet("maven"));
        try (InputStream in = Main.class.getResourceAsStream("/greeting.txt")) {
            System.out.println("resource " + new String(in.readAllBytes(), StandardCharsets.UTF_8).trim());
        }
    }

    static int add(int a, int b) {
        return a + b;
    }
}
