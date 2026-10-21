// Type variables with several bounds (`T extends A & B`): a value erases to the first bound and
// is cast to a later one where it calls that bound's method, as `Collections.max` does with
// `T extends Object & Comparable<? super T>`.
public class GenericIntersectionBounds {
    interface Named {
        String name();
    }

    interface Weighed {
        int weight();
    }

    abstract static class Shape {
        abstract double area();
    }

    static class Square extends Shape implements Named, Weighed {
        final int side;
        Square(int side) { this.side = side; }
        double area() { return side * side; }
        public String name() { return "square " + side; }
        public int weight() { return side * 10; }
    }

    static class Disc extends Shape implements Named, Weighed {
        final int r;
        Disc(int r) { this.r = r; }
        double area() { return 3 * r * r; }
        public String name() { return "disc " + r; }
        public int weight() { return r * 7; }
    }

    static class Item implements Named, Comparable<Item> {
        final String name;
        final int rank;
        Item(String name, int rank) { this.name = name; this.rank = rank; }
        public String name() { return name; }
        public int compareTo(Item o) { return Integer.compare(rank, o.rank); }
    }

    static <T extends Object & Comparable<? super T>> T max(T a, T b, T c) {
        T best = a;
        if (b.compareTo(best) > 0) best = b;
        if (c.compareTo(best) > 0) best = c;
        return best;
    }

    static <T extends Shape & Named & Weighed> String describe(T t) {
        return t.name() + ": area " + t.area() + ", weight " + t.weight();
    }

    static <T extends Named & Comparable<T>> String larger(T a, T b) {
        T winner = a.compareTo(b) >= 0 ? a : b;
        return winner.name();
    }

    static class Leader<T extends Named & Comparable<T>> {
        T best;

        void offer(T t) {
            if (best == null || t.compareTo(best) > 0) {
                best = t;
            }
        }

        String leader() {
            return best == null ? "nobody" : best.name();
        }
    }

    public static void main(String[] args) {
        System.out.println(max("pear", "apple", "plum"));
        System.out.println(max(3, 11, 7));
        System.out.println(max(new Item("a", 2), new Item("b", 5), new Item("c", 1)).name());
        System.out.println(describe(new Square(3)));
        System.out.println(describe(new Disc(2)));
        System.out.println(larger(new Item("low", 1), new Item("high", 9)));
        Leader<Item> leader = new Leader<>();
        System.out.println(leader.leader());
        leader.offer(new Item("x", 4));
        leader.offer(new Item("y", 8));
        leader.offer(new Item("z", 6));
        System.out.println(leader.leader());
    }
}
