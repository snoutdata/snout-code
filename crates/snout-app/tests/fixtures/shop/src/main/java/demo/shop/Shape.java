package demo.shop;

/** Something with an area. */
public interface Shape {
    double area();

    default String describe() {
        return "A shape of area " + area();
    }
}
