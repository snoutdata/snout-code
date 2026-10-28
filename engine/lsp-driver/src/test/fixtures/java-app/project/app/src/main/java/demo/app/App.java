package demo.app;

import demo.lib.Lib;
import demo.shared.Greeting;

public final class App {
    private App() {
    }

    public static Greeting greet(String name) {
        return new Greeting(Lib.hello(name));
    }

    public static void main(String[] args) {
        System.out.println(greet("world").shout());
    }
}
