package demo;

public class App {
    public static void main(String[] args) {
        int total = 0;
        for (int i = 1; i <= 3; i++) {
            total = Greeter.add(total, i);
        }
        System.out.println(Greeter.greet("remote") + " total " + total);
    }
}
