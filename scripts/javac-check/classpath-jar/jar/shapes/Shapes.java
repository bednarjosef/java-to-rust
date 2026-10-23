package shapes;

import java.util.ArrayList;
import java.util.List;

public final class Shapes {
    private Shapes() {
    }

    public record Square(double side) implements Shape {
        public double area() {
            return side * side;
        }
    }

    public enum Unit { CM, M }

    public static <T extends Shape> List<T> largerThan(List<T> shapes, double area) {
        List<T> out = new ArrayList<>();
        for (T shape : shapes) {
            if (shape.area() > area) {
                out.add(shape);
            }
        }
        return out;
    }
}
