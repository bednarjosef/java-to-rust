package shapes;

public interface Shape {
    int SIDES_OF_SQUARE = 4;

    double area();

    default String describe() {
        return getClass().getSimpleName() + " of area " + area();
    }
}
