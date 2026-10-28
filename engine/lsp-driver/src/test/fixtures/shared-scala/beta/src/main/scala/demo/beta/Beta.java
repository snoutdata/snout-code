package demo.beta;

import demo.shared.Tone;
import demo.shared.Words;

public final class Beta {
    private Beta() {
    }

    public static String all() {
        return BetaTune.tune() + Tone.loud("x") + Words.twice("y");
    }
}
