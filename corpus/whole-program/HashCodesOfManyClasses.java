import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Set;

/**
 * hashCode and System.identityHashCode of objects of many classes, some overriding hashCode and
 * some keeping Object's, through Object: as keys of HashMaps, HashSets and IdentityHashMaps, and
 * after their fields change, on this thread and another.
 */
public class HashCodesOfManyClasses {
    static class Plain {}

    static class Counter {
        int count;
    }

    static final class Shared {
        int hits;
    }

    static class Point {
        final int x;
        final int y;

        Point(int x, int y) {
            this.x = x;
            this.y = y;
        }

        @Override
        public boolean equals(Object o) {
            return o instanceof Point p && p.x == x && p.y == y;
        }

        @Override
        public int hashCode() {
            return 31 * x + y;
        }
    }

    static class Inherits extends Point {
        Inherits(int x, int y) {
            super(x, y);
        }
    }

    static class Labeled extends Point {
        final String label;

        Labeled(int x, int y, String label) {
            super(x, y);
            this.label = label;
        }

        @Override
        public boolean equals(Object o) {
            return o instanceof Labeled l && super.equals(l) && l.label.equals(label);
        }

        @Override
        public int hashCode() {
            return super.hashCode() * 31 + label.hashCode();
        }
    }

    static class Constant {
        @Override
        public int hashCode() {
            return 42;
        }
    }

    record Pair(Object first, Object second) {}

    enum Color {
        RED,
        GREEN,
        BLUE
    }

    interface Shape {
        int area();
    }

    static class Square implements Shape {
        final int side;

        Square(int side) {
            this.side = side;
        }

        public int area() {
            return side * side;
        }
    }

    static class Circle implements Shape {
        final int radius;

        Circle(int radius) {
            this.radius = radius;
        }

        public int area() {
            return 3 * radius * radius;
        }

        @Override
        public boolean equals(Object o) {
            return o instanceof Circle c && c.radius == radius;
        }

        @Override
        public int hashCode() {
            return radius;
        }
    }

    /** Classes enough that Object's dispatchers run each arm in a function of its own. */
    static class Many0 {}

    static class Many1 {
        @Override
        public int hashCode() {
            return 1007;
        }
    }

    static class Many2 {}

    static class Many3 {
        @Override
        public int hashCode() {
            return 3007;
        }
    }

    static class Many4 {}

    static class Many5 {
        @Override
        public int hashCode() {
            return 5007;
        }
    }

    static class Many6 {}

    static class Many7 {
        @Override
        public int hashCode() {
            return 7007;
        }
    }

    static class Many8 {}

    static class Many9 {
        @Override
        public int hashCode() {
            return 9007;
        }
    }

    static class Many10 {}

    static class Many11 {
        @Override
        public int hashCode() {
            return 11007;
        }
    }

    static class Many12 {}

    static class Many13 {
        @Override
        public int hashCode() {
            return 13007;
        }
    }

    static class Many14 {}

    static class Many15 {
        @Override
        public int hashCode() {
            return 15007;
        }
    }

    static class Many16 {}

    static class Many17 {
        @Override
        public int hashCode() {
            return 17007;
        }
    }

    static class Many18 {}

    static class Many19 {
        @Override
        public int hashCode() {
            return 19007;
        }
    }

    static class Many20 {}

    static class Many21 {
        @Override
        public int hashCode() {
            return 21007;
        }
    }

    static class Many22 {}

    static class Many23 {
        @Override
        public int hashCode() {
            return 23007;
        }
    }

    static class Many24 {}

    static class Many25 {
        @Override
        public int hashCode() {
            return 25007;
        }
    }

    static class Many26 {}

    static class Many27 {
        @Override
        public int hashCode() {
            return 27007;
        }
    }

    static class Many28 {}

    static class Many29 {
        @Override
        public int hashCode() {
            return 29007;
        }
    }

    static class Many30 {}

    static class Many31 {
        @Override
        public int hashCode() {
            return 31007;
        }
    }

    static class Many32 {}

    static class Many33 {
        @Override
        public int hashCode() {
            return 33007;
        }
    }

    static class Many34 {}

    static class Many35 {
        @Override
        public int hashCode() {
            return 35007;
        }
    }

    static class Many36 {}

    static class Many37 {
        @Override
        public int hashCode() {
            return 37007;
        }
    }

    static class Many38 {}

    static class Many39 {
        @Override
        public int hashCode() {
            return 39007;
        }
    }

    static Object many(int k) {
        return switch (k) {
            case 0 -> new Many0();
            case 1 -> new Many1();
            case 2 -> new Many2();
            case 3 -> new Many3();
            case 4 -> new Many4();
            case 5 -> new Many5();
            case 6 -> new Many6();
            case 7 -> new Many7();
            case 8 -> new Many8();
            case 9 -> new Many9();
            case 10 -> new Many10();
            case 11 -> new Many11();
            case 12 -> new Many12();
            case 13 -> new Many13();
            case 14 -> new Many14();
            case 15 -> new Many15();
            case 16 -> new Many16();
            case 17 -> new Many17();
            case 18 -> new Many18();
            case 19 -> new Many19();
            case 20 -> new Many20();
            case 21 -> new Many21();
            case 22 -> new Many22();
            case 23 -> new Many23();
            case 24 -> new Many24();
            case 25 -> new Many25();
            case 26 -> new Many26();
            case 27 -> new Many27();
            case 28 -> new Many28();
            case 29 -> new Many29();
            case 30 -> new Many30();
            case 31 -> new Many31();
            case 32 -> new Many32();
            case 33 -> new Many33();
            case 34 -> new Many34();
            case 35 -> new Many35();
            case 36 -> new Many36();
            case 37 -> new Many37();
            case 38 -> new Many38();
            case 39 -> new Many39();
            default -> new Object();
        };
    }

    /** Whether `o` is of a class that keeps Object's hashCode. */
    static boolean identityHashed(Object o) {
        return o instanceof Plain
                || o instanceof Counter
                || o instanceof Shared
                || o instanceof Color
                || o instanceof Square
                || o instanceof int[]
                || o.getClass() == Object.class;
    }

    static String name(Object o) {
        return o instanceof int[] ? "int[]" : o.getClass().getSimpleName();
    }

    public static void main(String[] args) throws InterruptedException {
        Counter counter = new Counter();
        Shared shared = new Shared();
        List<Object> objects = new ArrayList<>();
        objects.add(new Plain());
        objects.add(new Plain());
        objects.add(counter);
        objects.add(new Counter());
        objects.add(shared);
        objects.add(new Shared());
        objects.add(new Point(1, 2));
        objects.add(new Point(1, 2));
        objects.add(new Inherits(1, 2));
        objects.add(new Inherits(2, 1));
        objects.add(new Labeled(1, 2, "a"));
        objects.add(new Labeled(1, 2, "b"));
        objects.add(new Constant());
        objects.add(new Constant());
        objects.add(new Pair(new Point(3, 4), "x"));
        objects.add(new Pair(new Point(3, 4), "x"));
        objects.add(Color.RED);
        objects.add(Color.BLUE);
        objects.add(new Square(2));
        objects.add(new Circle(2));
        objects.add(new Circle(2));
        objects.add(new int[] {1, 2});
        objects.add(new int[] {1, 2});
        objects.add(new Object());
        objects.add("text");
        objects.add(7);
        objects.add(7L);
        objects.add(2.5);
        objects.add('c');
        objects.add(true);

        // Each hash, twice, and whether it is the identity's for a class that keeps Object's.
        for (Object o : objects) {
            int hash = o.hashCode();
            StringBuilder line = new StringBuilder(name(o));
            line.append(hash == o.hashCode() ? " stable" : " changed");
            line.append(System.identityHashCode(o) == System.identityHashCode(o) ? " stable" : " changed");
            if (identityHashed(o)) {
                line.append(" identity ").append(hash == System.identityHashCode(o));
            } else if (!(o instanceof Pair)) {
                line.append(" value ").append(hash);
            }
            System.out.println(line);
        }
        Pair p = (Pair) objects.get(14);
        System.out.println("equal pairs hash alike " + (p.hashCode() == objects.get(15).hashCode()));

        Map<Object, Integer> byValue = new HashMap<>();
        Map<Object, Integer> byIdentity = new IdentityHashMap<>();
        Set<Object> set = new HashSet<>();
        for (int i = 0; i < objects.size(); i++) {
            Object o = objects.get(i);
            byValue.put(o, i);
            set.add(o);
            if (identityHashed(o) || o instanceof Point || o instanceof Constant || o instanceof Circle) {
                byIdentity.put(o, i);
            }
        }
        System.out.println("sizes " + byValue.size() + " " + set.size() + " " + byIdentity.size());
        StringBuilder found = new StringBuilder("found");
        for (Object o : objects) {
            found.append(' ').append(byValue.get(o)).append('/').append(byIdentity.get(o));
        }
        System.out.println(found);

        // A changed field changes no identity hash: the objects are found where they were put.
        counter.count = 9;
        Thread worker = new Thread(() -> {
            for (int i = 0; i < 1000; i++) {
                synchronized (shared) {
                    shared.hits++;
                }
            }
        });
        worker.start();
        worker.join();
        synchronized (shared) {
            System.out.println("hits " + shared.hits);
        }
        System.out.println("after changes " + byValue.get(counter) + " " + byIdentity.get(counter) + " "
                + byValue.get(shared) + " " + byIdentity.get(shared) + " " + set.contains(shared));
        System.out.println("fresh objects " + byValue.get(new Counter()) + " " + byIdentity.get(new Point(1, 2))
                + " " + byValue.get(new Point(1, 2)) + " " + byValue.get(new Inherits(2, 1)));

        // An object of each of many classes, half of which keep Object's hashCode.
        Map<Object, Integer> ofMany = new HashMap<>();
        Map<Object, Integer> ofManyByIdentity = new IdentityHashMap<>();
        StringBuilder hashes = new StringBuilder("many classes");
        for (int k = 0; k < 40; k++) {
            Object o = many(k);
            ofMany.put(o, k);
            ofManyByIdentity.put(o, k);
            hashes.append(' ').append(k % 2 == 0 ? (o.hashCode() == System.identityHashCode(o) ? "i" : "?") : o.hashCode());
        }
        System.out.println(hashes);
        int back = 0;
        for (Map.Entry<Object, Integer> e : ofManyByIdentity.entrySet()) {
            back += ofMany.get(e.getKey()).equals(e.getValue()) ? 1 : 0;
        }
        System.out.println("found again " + back + " of " + ofMany.size());

        // Many keys of a few classes, through the same maps.
        Map<Object, Integer> many = new HashMap<>();
        List<Object> keys = new ArrayList<>();
        for (int i = 0; i < 300; i++) {
            Object key = switch (i % 6) {
                case 0 -> new Plain();
                case 1 -> new Point(i, -i);
                case 2 -> new Circle(i);
                case 3 -> new Counter();
                case 4 -> Color.values()[i % 3];
                default -> new Labeled(i, i, "k" + i);
            };
            keys.add(key);
            many.merge(key, 1, Integer::sum);
        }
        int total = 0;
        for (Object key : keys) {
            total += many.get(key);
        }
        System.out.println("many " + many.size() + " " + total);
        Object nothing = keys.size() > 1000 ? keys.get(0) : null;
        System.out.println("nulls " + System.identityHashCode(nothing) + " " + Objects.hashCode(nothing));
    }
}
