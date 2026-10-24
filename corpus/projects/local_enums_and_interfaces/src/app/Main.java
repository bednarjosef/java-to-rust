package app;

// Local enums and interfaces in a unit of a package that declares no record.
public class Main {
    int base = 10;

    static String holder() {
        // A holder whose constant is made the first time the method reads it.
        interface Lazy {
            StringBuilder INSTANCE = made();
        }
        return Lazy.INSTANCE.append('!').toString();
    }

    static StringBuilder made() {
        System.out.println("made");
        return new StringBuilder("lazy");
    }

    int score(String word) {
        enum Kind {
            SHORT(1), LONG(3);
            final int points;
            Kind(int points) { this.points = points; }
            static Kind of(String w) { return w.length() < 4 ? SHORT : LONG; }
        }
        Kind kind = Kind.of(word);
        System.out.println(kind + " " + kind.ordinal() + " " + Kind.valueOf("LONG").name());
        return base + kind.points * word.length();
    }

    static int shapes(int side) {
        interface Shape {
            int area();
            default String describe() { return getClass().getSimpleName() + " " + area(); }
        }
        class Square implements Shape {
            public int area() { return side * side; }
        }
        Shape[] all = { new Square(), () -> side * 2 };
        int sum = 0;
        for (Shape shape : all) {
            if (shape instanceof Square) System.out.println(shape.describe());
            sum += shape.area();
        }
        return sum;
    }

    public static void main(String[] args) {
        System.out.println("start");
        System.out.println(holder());
        System.out.println(holder());
        Main o = new Main();
        System.out.println(o.score("cat"));
        System.out.println(o.score("giraffe"));
        System.out.println(shapes(3));
        System.out.println(Other.coin());
    }
}
