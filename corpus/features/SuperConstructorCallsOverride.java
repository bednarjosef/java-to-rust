// A superclass's constructor calls a method that its subclass overrides, which reads the
// subclass's fields: they are still zero, null and false there, as their initializers and the
// subclass's constructor run after the superclass's constructor.
public class SuperConstructorCallsOverride {
    abstract static class Shape {
        final String summary;
        int sides;

        Shape(int sides) {
            this.sides = sides;
            summary = describe();
            System.out.println("Shape(" + sides + "): " + summary);
        }

        abstract String describe();

        double area() {
            return 0;
        }
    }

    static class Square extends Shape {
        int size = 4;
        String name = "square";
        boolean filled = true;
        int[] corners = new int[4];
        Square partner;

        Square(int size) {
            super(4);
            System.out.println("Square(" + size + ") before: " + describe());
            this.size = size;
            partner = this;
            System.out.println("Square(" + size + ") after: " + describe());
        }

        String describe() {
            return name + " size " + size + " filled " + filled + " corners "
                    + (corners == null ? "null" : String.valueOf(corners.length)) + " partner "
                    + (partner == null ? "null" : "set");
        }

        @Override
        double area() {
            return size * size;
        }
    }

    static class Circle extends Shape {
        double radius;

        Circle(double radius) {
            super(0);
            this.radius = radius;
        }

        String describe() {
            return "circle r=" + radius + " area " + area();
        }

        @Override
        double area() {
            return 3 * radius * radius;
        }
    }

    public static void main(String[] args) {
        Shape s = new Square(3);
        System.out.println(s.summary);
        System.out.println(s.describe());
        System.out.println(s.area());
        Shape c = new Circle(2);
        System.out.println(c.summary);
        System.out.println(c.describe() + " sides " + c.sides);
    }
}
