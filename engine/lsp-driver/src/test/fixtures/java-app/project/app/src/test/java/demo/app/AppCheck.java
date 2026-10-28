package demo.app;

import demo.lib.Lib;

final class AppCheck {
    private AppCheck() {
    }

    static boolean works() {
        return App.greet("x").text().equals(Lib.hello("x"));
    }
}
