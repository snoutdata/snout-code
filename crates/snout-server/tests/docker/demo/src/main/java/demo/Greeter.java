package demo;

public final class Greeter {
    private Greeter() {}

    public static String greet(String name) {
        return "Hello, " + name;
    }

    static int add(int a, int b) {
        return a + b;
    }
}
