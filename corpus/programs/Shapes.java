public class Shapes {
    static abstract class Base {
        protected String name;
        int uses;
        Base(String name) { this.name = name; }
        String getName() { uses++; return name; }
        abstract double area();
        public String toString() { return name + ":" + area(); }
    }
    static class Circle extends Base {
        final double r;
        Circle(double r) { super("circle"); this.r = r; }
        double area() { return 3.0 * r * r; }
        public String toString() { return "C" + super.toString(); }
    }
    static class Rect extends Base {
        double w, h;
        Rect(double w, double h) { super("rect"); this.w = w; this.h = h; }
        double area() { return w * h; }
    }
    public static void main(String[] args) {
        Base[] shapes = { new Circle(1.5), new Rect(2, 3) };
        double total = 0;
        for (Base b : shapes) {
            total += b.area();
            System.out.println(b + " " + b.getName());
        }
        System.out.println(total);
        Base x = shapes[0];
        System.out.println(x == shapes[0]);
        System.out.println(x == shapes[1]);
        System.out.println(shapes[1].uses + " " + x.name);
        Circle c = new Circle(2);
        Base y = c;
        System.out.println(y == c);
        System.out.println(y);
    }
}
