// `==` and `!=` between values of which neither type is a subtype of the other: a class and
// an interface it does not implement (a subclass might), and two interfaces; also with a
// static field and null. Nothing else in the program uses `Object`.
public class UnrelatedComparisons {
    interface Named {
        String name();
    }
    interface Sized {
        int size();
    }
    static class Shape implements Named {
        int x = 1;
        public String name() { return "shape"; }
    }
    static class Box extends Shape implements Sized {
        public int size() { return x + 1; }
    }
    static class Line extends Shape {
    }
    static class Tag implements Sized {
        public int size() { return 7; }
    }
    static Sized sSized;

    public static void main(String[] args) {
        Line line = new Line();
        Sized sized = new Box();
        Sized tag = new Tag();
        Named named = new Line();
        Box box = new Box();
        sSized = box;
        System.out.println(line == sized);
        System.out.println(sized != line);
        System.out.println(tag == named);
        System.out.println(named == sized);
        System.out.println(box == sSized);
        System.out.println(sSized == box);
        System.out.println(line == sSized);
        Shape shape = box;
        System.out.println(shape == sSized);
        System.out.println(sSized == shape);
        Sized maybe = null;
        System.out.println(line == maybe);
        line.x = 4;
        System.out.println(named.name() + sized.size() + tag.size());
    }
}
