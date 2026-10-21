public class ObjectLevelInheritance {
    static Shape lastRegistered;
    static int registrations;

    static void register(Shape s) {
        lastRegistered = s;
        registrations++;
    }

    static abstract class Shape {
        String tag;
        Shape(String tag) { this.tag = tag; }
        abstract double size();
        String describe() { return tag + " of size " + size(); }
        void publish() { register(this); }
        Shape bigger(Shape other) { return other.size() > size() ? other : this; }
    }

    static abstract class Polygon extends Shape {
        int corners;
        Polygon(String tag, int corners) { super(tag); this.corners = corners; }
        String describe() { return corners + "-gon, " + super.describe(); }
    }

    static class Square extends Polygon {
        double side;
        Square(double side) { super("square", 4); this.side = side; }
        double size() { return side * side; }
        String label() { return "[" + describe() + "]"; }
    }

    static class Triangle extends Polygon {
        double b, h;
        Triangle(double b, double h) { super("triangle", 3); this.b = b; this.h = h; }
        double size() { return b * h / 2; }
        String describe() { return "pointy " + super.describe(); }
    }

    static class Blob extends Shape {
        double mass;
        Blob(double mass) { super("blob"); this.mass = mass; }
        double size() { return mass; }
        void grow() { mass *= 2; publish(); }
    }

    public static void main(String[] args) {
        Square sq = new Square(3);
        Triangle tr = new Triangle(4, 5);
        Blob bl = new Blob(7);
        Shape[] all = { sq, tr, bl };
        for (Shape s : all) {
            System.out.println(s.describe());
            s.publish();
        }
        System.out.println(registrations + " " + lastRegistered.describe());
        System.out.println(sq.label());
        bl.grow();
        System.out.println(lastRegistered.size() + " " + (lastRegistered == bl));
        Polygon p = tr;
        System.out.println(p.corners + " " + p.describe() + " " + p.bigger(sq).describe());
        Shape big = all[0];
        for (Shape s : all) big = big.bigger(s);
        System.out.println(big.tag);
        p.corners = 30;
        Polygon q = sq;
        q.corners += p.corners;
        System.out.println(tr.corners + " " + sq.corners);
    }
}
