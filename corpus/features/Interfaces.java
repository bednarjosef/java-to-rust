public class Interfaces {
    interface Shape {
        int SIDES_UNKNOWN = -1;
        double area();
        default int sides() { return SIDES_UNKNOWN; }
        default String describe() { return name() + " with area " + area() + " and " + sides() + " sides"; }
        String name();
        static Shape bigger(Shape a, Shape b) { return a.area() >= b.area() ? a : b; }
    }

    interface Named {
        String name();
        default String greet() { return "hello " + name(); }
    }

    interface Polygon extends Shape {
        int corners();
        default int sides() { return corners(); }
    }

    static class Circle implements Shape, Named {
        final double r;
        Circle(double r) { this.r = r; }
        public double area() { return 3.0 * r * r; }
        public String name() { return "circle"; }
    }

    static class Rect implements Polygon {
        double w, h;
        Rect(double w, double h) { this.w = w; this.h = h; }
        public double area() { return w * h; }
        public String name() { return "rect"; }
        public int corners() { return 4; }
        void grow() { w += 1; }
    }

    static class Square extends Rect implements Named {
        Square(double s) { super(s, s); }
        public String name() { return "square"; }
        public String greet() { return "hi " + name(); }
    }

    static double total(Shape[] shapes) {
        double sum = 0;
        for (Shape s : shapes) sum += s.area();
        return sum;
    }

    public static void main(String[] args) {
        Rect r = new Rect(2, 3);
        Square q = new Square(2);
        Shape[] shapes = { new Circle(1), r, q };
        for (Shape s : shapes) System.out.println(s.describe());
        System.out.println(total(shapes));
        r.grow();
        q.grow();
        System.out.println(total(shapes));
        Named[] names = { new Circle(2), q };
        for (Named n : names) System.out.println(n.greet());
        Polygon p = q;
        System.out.println(p.corners() + " " + p.sides() + " " + p.name());
        Shape big = Shape.bigger(r, q);
        System.out.println(big.name() + " " + (big == r) + " " + (big == q));
        System.out.println(Shape.SIDES_UNKNOWN);
    }
}
