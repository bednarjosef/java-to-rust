import java.util.HashMap;
import java.util.HashSet;
import java.util.Map;
import java.util.Set;

/// Records whose components are classes of the program: a class that keeps `Object`'s hashCode
/// and equals (identity), one that overrides them, an abstract class whose subclasses differ, and
/// a class whose equals is false even for itself, so that a record holding it equals only
/// itself. Null components hash to 0 and equal only null, and the components are compared from
/// the last to the first, which the counts of `equals` calls show.
public class RecordClassComponents {
    static int equalsCalls;
    static int hashCalls;

    static final class Key {
        final String name;

        Key(String name) {
            this.name = name;
        }
    }

    static final class Name {
        final String text;

        Name(String text) {
            this.text = text;
        }

        @Override
        public boolean equals(Object o) {
            equalsCalls++;
            return o instanceof Name n && n.text.equals(text);
        }

        @Override
        public int hashCode() {
            hashCalls++;
            return text.hashCode();
        }
    }

    abstract static class Shape {
        abstract double area();
    }

    static final class Square extends Shape {
        final int side;

        Square(int side) {
            this.side = side;
        }

        double area() {
            return side * side;
        }

        @Override
        public boolean equals(Object o) {
            equalsCalls++;
            return o instanceof Square s && s.side == side;
        }

        @Override
        public int hashCode() {
            hashCalls++;
            return side;
        }
    }

    static final class Dot extends Shape {
        double area() {
            return 0;
        }
    }

    static final class Never {
        @Override
        public boolean equals(Object o) {
            equalsCalls++;
            return false;
        }

        @Override
        public int hashCode() {
            return 7;
        }
    }

    record Tag(Key registry, Name location) {}

    record Placed(Shape shape, int x) {}

    record Odd(Never never) {}

    public static void main(String[] args) {
        Key blocks = new Key("blocks");
        Key items = new Key("items");
        Tag a = new Tag(blocks, new Name("stone"));
        Tag b = new Tag(blocks, new Name("stone"));
        Tag c = new Tag(items, new Name("stone"));
        Tag d = new Tag(blocks, new Name("dirt"));
        System.out.println(a.equals(b) + " " + a.equals(c) + " " + a.equals(d) + " calls " + equalsCalls);
        System.out.println((a.hashCode() == b.hashCode()) + " calls " + hashCalls);
        Tag nulls = new Tag(null, null);
        System.out.println(nulls.hashCode() + " " + nulls.equals(new Tag(null, null)) + " " + nulls.equals(a)
                + " " + a.equals(nulls) + " calls " + equalsCalls);
        Tag half = new Tag(blocks, null);
        System.out.println(half.equals(new Tag(blocks, null)) + " " + half.equals(a) + " calls " + equalsCalls);

        Set<Tag> tags = new HashSet<>();
        tags.add(a);
        tags.add(c);
        System.out.println(tags.contains(b) + " " + tags.contains(d) + " " + tags.size());

        Placed p = new Placed(new Square(3), 1);
        Placed q = new Placed(new Square(3), 1);
        Placed r = new Placed(new Dot(), 1);
        Dot dot = new Dot();
        System.out.println(p.equals(q) + " " + p.equals(r) + " " + new Placed(dot, 2).equals(new Placed(dot, 2))
                + " " + new Placed(dot, 2).equals(new Placed(new Dot(), 2)) + " calls " + equalsCalls);
        Map<Placed, String> names = new HashMap<>();
        names.put(p, "square");
        System.out.println(names.get(q) + " " + names.get(r) + " " + p.shape().area());

        Never never = new Never();
        Odd odd = new Odd(never);
        System.out.println(odd.equals(odd) + " " + odd.equals(new Odd(never)) + " " + odd.equals(new Odd(new Never()))
                + " " + (odd.hashCode() == new Odd(new Never()).hashCode()) + " calls " + equalsCalls);
    }
}
