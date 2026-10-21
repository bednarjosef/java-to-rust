// `super.clone()` in a class tree: Object's clone copies every field of the object's runtime
// class, its subclass's fields too, and the copy is an object of that class.
public class CloneInClassTrees {
    // A Cloneable superclass whose clone runs on objects of its subclasses, as EnumSet's does on
    // a RegularEnumSet.
    abstract static class Shape implements Cloneable {
        String name;
        int[] origin = {0, 0};

        Shape(String name) { this.name = name; }

        abstract double area();

        Shape copy() {
            try {
                return (Shape) super.clone();
            } catch (CloneNotSupportedException e) {
                throw new AssertionError(e);
            }
        }

        String describe() { return name + " at " + origin[0] + "," + origin[1] + " area " + area(); }
    }

    static class Circle extends Shape {
        double r;

        Circle(double r) {
            super("circle");
            this.r = r;
        }

        double area() { return 3 * r * r; }
    }

    static class Rect extends Shape {
        double w, h;

        Rect(double w, double h) {
            super("rect");
            this.w = w;
            this.h = h;
        }

        double area() { return w * h; }
    }

    // A subclass's own clone calling its superclass's, which calls Object's: the copy has the
    // subclass's fields, and the subclass then copies its array, as JumboEnumSet does.
    static class Square extends Rect {
        int[] marks = {1, 2};

        Square(double side) { super(side, side); }

        @Override
        Square copy() {
            Square s = (Square) super.copy();
            s.marks = s.marks.clone();
            return s;
        }
    }

    // A superclass that is not Cloneable, whose clone clears a cache, as AbstractMap's does; the
    // subclass is Cloneable.
    static class Table {
        String cache = "cached";
        int size;

        @Override
        protected Object clone() throws CloneNotSupportedException {
            Table t = (Table) super.clone();
            t.cache = null;
            return t;
        }
    }

    static class NamedTable extends Table implements Cloneable {
        String title;

        NamedTable(String title) { this.title = title; }

        NamedTable copy() throws CloneNotSupportedException { return (NamedTable) clone(); }
    }

    // A subclass that is not Cloneable: Object's clone throws for its objects.
    static class PlainTable extends Table {
        Object copy() throws CloneNotSupportedException { return clone(); }
    }

    public static void main(String[] args) throws CloneNotSupportedException {
        Shape c = new Circle(2);
        Shape c2 = c.copy();
        ((Circle) c2).r = 1;
        c2.origin[0] = 5;
        System.out.println(c.describe() + " | " + c2.describe());
        System.out.println((c2 instanceof Circle) + " " + (c2 != c) + " " + (c2.origin == c.origin));

        Rect r = new Rect(2, 3);
        Rect r2 = (Rect) r.copy();
        r2.w = 10;
        r2.name = "wide";
        System.out.println(r.describe() + " | " + r2.describe());

        Square s = new Square(4);
        Square s2 = s.copy();
        s2.marks[0] = 9;
        s2.h = 1;
        System.out.println(s.describe() + " " + s.marks[0] + " | " + s2.describe() + " " + s2.marks[0]);
        Shape sq = s;
        System.out.println(sq.copy().getClass().getSimpleName());

        NamedTable t = new NamedTable("t");
        t.size = 3;
        NamedTable t2 = t.copy();
        t2.size++;
        System.out.println(t.cache + " " + t.size + " " + t.title + " | " + t2.cache + " " + t2.size + " " + t2.title);

        PlainTable p = new PlainTable();
        try {
            p.copy();
            System.out.println("copied");
        } catch (CloneNotSupportedException e) {
            System.out.println("refused: " + e.getMessage());
        }
    }
}
