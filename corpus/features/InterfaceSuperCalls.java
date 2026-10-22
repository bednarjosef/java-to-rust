// An interface's default method that calls the default method it overrides, of an interface
// it extends, through `Iface.super.m()`, and a class that does the same.
public class InterfaceSuperCalls {
    interface Shape {
        int sides();

        default String describe() {
            return "a shape of " + sides() + " sides";
        }

        default int weight(int scale) {
            return sides() * scale;
        }
    }

    interface Named extends Shape {
        String name();

        @Override
        default String describe() {
            return name() + ", " + Shape.super.describe();
        }

        @Override
        default int weight(int scale) {
            return Shape.super.weight(scale) + 1;
        }
    }

    static class Square implements Named {
        public int sides() {
            return 4;
        }

        public String name() {
            return "square";
        }
    }

    static class Triangle implements Named {
        public int sides() {
            return 3;
        }

        public String name() {
            return "triangle";
        }

        @Override
        public String describe() {
            return "[" + Named.super.describe() + "]";
        }
    }

    public static void main(String[] args) {
        Shape[] shapes = {new Square(), new Triangle()};
        for (Shape s : shapes) {
            System.out.println(s.describe() + " weighing " + s.weight(10));
        }
    }
}
