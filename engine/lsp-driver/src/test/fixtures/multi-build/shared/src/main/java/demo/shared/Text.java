package demo.shared;

import demo.helper.Helper;

public final class Text {
    private Text() {
    }

    public static String shout(String text) {
        return Helper.upper(text) + "!";
    }
}
