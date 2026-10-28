package demo.shared;

public record Greeting(String text) {
    public String shout() {
        return text.toUpperCase();
    }
}
