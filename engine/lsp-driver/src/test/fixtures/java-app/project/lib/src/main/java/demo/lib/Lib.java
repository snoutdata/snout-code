package demo.lib;

import demo.lib.gen.Version;

public final class Lib {
    private Lib() {
    }

    public static String hello(String name) {
        return "hello " + name;
    }

    public static String version() {
        return Version.VALUE;
    }
}
