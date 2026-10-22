// A class whose superclass's constructor calls a method that subclasses override (as
// `AbstractMap`'s tree is built, under `TreeMap`), with field initializers and a constructor
// that the program never calls (`TreeMap(SortedMap)`): that constructor builds nothing.
public class UncalledLeafConstructors {
    static final StringBuilder log = new StringBuilder();

    abstract static class Shape {
        final String kind;

        Shape(String kind) {
            this.kind = kind;
            log.append("shape ").append(kind).append(" ").append(describe()).append("\n");
        }

        abstract String describe();
    }

    static final class Square extends Shape {
        int side = 2;
        int[] marks = new int[0];

        Square() {
            super("square");
        }

        Square(int side) {
            super("square");
            this.side = side;
        }

        Square(Square other) {
            super(other.kind);
            this.side = other.side;
            marks = other.marks.clone();
        }

        String describe() {
            return "side " + side + " marks " + (marks == null ? "unset" : String.valueOf(marks.length));
        }
    }

    static final class Circle extends Shape {
        Circle() {
            super("circle");
        }

        String describe() {
            return "round";
        }
    }

    public static void main(String[] args) {
        Square small = new Square();
        Square large = new Square(9);
        large.marks = new int[] {4};
        Shape circle = new Circle();
        System.out.println(small.describe() + "; " + large.describe() + "; " + circle.describe());
        System.out.print(log);
    }
}
