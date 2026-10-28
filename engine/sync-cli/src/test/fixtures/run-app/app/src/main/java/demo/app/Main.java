package demo.app;

import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;

public final class Main {
    private Main() {
    }

    public static void main(String[] args) throws IOException {
        int total = 0;
        for (int i = 1; i <= 3; i++) {
            total = add(total, i);
        }
        System.out.println("total " + total);
        System.out.println(Greeter.greet("debugger"));
        try (InputStream in = Main.class.getResourceAsStream("/greeting.txt")) {
            System.out.println("resource " + (in == null ? "missing" : new String(in.readAllBytes(), StandardCharsets.UTF_8).trim()));
        }
    }

    static int add(int a, int b) {
        return a + b;
    }
}
