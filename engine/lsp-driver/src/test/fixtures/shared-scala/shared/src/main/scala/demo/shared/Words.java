package demo.shared;

/** Java that uses the Scala beside it. */
public final class Words {
    private Words() {
    }

    public static String upper(String s) {
        return s.toUpperCase();
    }

    public static String twice(String s) {
        return Tone.loud(s) + Tone.loud(s);
    }
}
