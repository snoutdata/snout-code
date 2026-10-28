package demo.alpha;

import demo.shared.Tone;
import demo.shared.Words;

public final class Alpha {
    private Alpha() {
    }

    public static String all() {
        return AlphaTune.tune() + Tone.loud("x") + Words.twice("y");
    }
}
