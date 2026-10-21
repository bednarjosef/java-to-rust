// Overridden methods whose parameter is named `o` (`equals(Object o)`, `compareTo(Shape o)`,
// `scaled(int o)`), called on values of a class with subclasses through its dispatcher.
public class DispatchParameterNamedO {
    static class Shape implements Comparable<Shape> {
        int size;
        Shape(int size) {
            this.size = size;
        }
        public int compareTo(Shape o) {
            return size - o.size;
        }
        int scaled(int o) {
            return size * o;
        }
    }
    static class Square extends Shape {
        Square(int size) {
            super(size);
        }
        @Override
        public boolean equals(Object o) {
            return o instanceof Square s && s.size == size;
        }
        @Override
        public int hashCode() {
            return size;
        }
        @Override
        int scaled(int o) {
            return size + o;
        }
    }
    static class Line extends Shape {
        Line(int size) {
            super(size);
        }
        @Override
        public boolean equals(Object o) {
            return o instanceof Line l && l.size == size;
        }
        @Override
        public int hashCode() {
            return -size;
        }
    }

    public static void main(String[] args) {
        Shape[] shapes = {new Square(2), new Line(3), new Shape(4), new Square(2)};
        for (Shape a : shapes) {
            for (Shape b : shapes) {
                System.out.print(a.equals(b) + ":" + a.compareTo(b) + ":" + a.scaled(b.size) + " ");
            }
            System.out.println(a.hashCode() == a.hashCode());
        }
    }
}
