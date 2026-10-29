package demo.shop;

import java.util.ArrayList;
import java.util.List;

/** A shop that sells shapes. */
public class Shop {
    static final int LIMIT = 10;
    private final List<Shape> shapes = new ArrayList<>();
    protected String owner;
    int visits;

    public Shop(String owner) {
        this.owner = owner;
    }

    public void add(Shape shape) {
        if (shapes.size() < LIMIT) {
            shapes.add(shape);
        }
    }

    public double total() {
        double sum = 0;
        for (Shape s : shapes) {
            sum += s.area();
        }
        return sum;
    }

    private static String label(String name, int count, boolean loud) {
        String text = name + ": " + count;
        return loud ? text.toUpperCase() : text;
    }

    public static void main(String[] args) {
        Shop shop = new Shop("Ada");
        shop.add(new Circle(2.0));
        shop.add(new Square(3.0));
        int tax = 7;
        System.out.println(label("Total", 42, true) + " " + shop.total() * 1.25 + tax);
    }
}
