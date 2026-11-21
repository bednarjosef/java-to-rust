import java.util.HashMap;
import java.util.Map;
import java.util.Objects;

// Objects.equals, Objects.hashCode and Objects.hash written in place at each call: the identity
// shortcut before an equals that is not reflexive, null on either side, arguments with effects
// read once and in order before any hashCode runs, objects of a class, an interface and Object,
// strings, boxes, numbers and enums, a record whose components are objects, arrays, and values
// read from fields that change.
public class ObjectsEqualsAndHashCodeInPlace {
    interface Shape {
        int sides();
    }

    static final class Square implements Shape {
        final int size;

        Square(int size) {
            this.size = size;
        }

        public int sides() {
            return 4;
        }

        @Override
        public boolean equals(Object o) {
            compared++;
            return o instanceof Square s && s.size == size;
        }

        @Override
        public int hashCode() {
            hashed++;
            return size * 7;
        }
    }

    // An equals that is never true, not even for itself: Objects.equals(n, n) is still true.
    static final class Never {
        @Override
        public boolean equals(Object o) {
            compared++;
            return false;
        }

        @Override
        public int hashCode() {
            return 5;
        }
    }

    enum Color { RED, GREEN }

    record Tagged(Square square, Shape shape, String name, Color color) {}

    static int compared;
    static int hashed;
    static StringBuilder order = new StringBuilder();
    static Square held = new Square(3);
    static Shape shape = new Square(3);
    static Object anything = "text";

    static <T> T note(String label, T value) {
        order.append(label);
        return value;
    }

    public static void main(String[] args) {
        Never never = new Never();
        Square a = new Square(1);
        Square b = new Square(1);
        Square c = new Square(2);
        Square none = args.length > 5 ? a : null;
        System.out.println(Objects.equals(never, never) + " " + Objects.equals(never, new Never()) + " " + compared);
        System.out.println(Objects.equals(a, b) + " " + Objects.equals(a, c) + " " + Objects.equals(a, none) + " "
            + Objects.equals(none, a) + " " + Objects.equals(none, none) + " " + compared);
        System.out.println(Objects.equals(note("x", a), note("y", b)) + " " + Objects.equals(note("p", none), note("q", c))
            + " " + order + " " + compared);
        System.out.println(Objects.equals(held, a) + " " + Objects.equals(held, shape) + " " + Objects.equals(shape, held)
            + " " + Objects.equals(anything, "text") + " " + Objects.equals(anything, held) + " " + compared);
        held = null;
        System.out.println(Objects.equals(held, a) + " " + Objects.hashCode(held) + " " + Objects.hashCode(shape) + " "
            + Objects.hashCode(a) + " " + Objects.hashCode(none) + " " + hashed);
        System.out.println(Objects.equals("a", "a") + " " + Objects.equals((Integer) 300, (Integer) 300) + " "
            + Objects.equals((Object) 1, (Object) 1L) + " " + Objects.equals(Color.RED, Color.RED) + " "
            + Objects.equals(Color.RED, Color.GREEN) + " " + Objects.hashCode("abc") + " " + Objects.hashCode(42L) + " "
            + Objects.hashCode(1.5) + " " + (Objects.hashCode(Color.GREEN) == Color.GREEN.hashCode()));
        int[] xs = {1, 2};
        System.out.println(Objects.equals(xs, xs) + " " + Objects.equals(xs, new int[] {1, 2}));
        Tagged t = new Tagged(a, c, "t", Color.RED);
        Tagged u = new Tagged(b, new Square(2), "t", Color.RED);
        Tagged v = new Tagged(null, null, null, null);
        System.out.println(t.equals(u) + " " + (t.hashCode() == u.hashCode()) + " " + t.equals(v) + " " + v.equals(v) + " "
            + v.hashCode() + " " + compared + " " + hashed);
        System.out.println(Objects.hash() + " " + Objects.hash(1, 2L, 1.5, true, 'c', (short) 3) + " "
            + Objects.hash("a", null, (Integer) 7) + " "
            + (Objects.hash(Color.RED, a) == 31 * (31 + Color.RED.hashCode()) + a.hashCode()) + " "
            + Objects.hash(note("h", a), note("i", c), note("j", none)) + " " + order + " " + hashed);
        Map<Tagged, String> map = new HashMap<>();
        map.put(t, "first");
        System.out.println(map.get(u) + " " + map.get(v) + " " + map.containsKey(new Tagged(b, new Square(2), "t", Color.RED)));
    }
}
