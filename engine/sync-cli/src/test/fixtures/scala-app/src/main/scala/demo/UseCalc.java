package demo;

public final class UseCalc {
    private UseCalc() {
    }

    public static int run() {
        return Calc.twice(2) + new Other().name().length();
    }
}
