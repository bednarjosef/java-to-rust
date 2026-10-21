// Generic interfaces implemented with type arguments, and `Comparable`: the bridges that
// erasure needs, and type variables bounded by `Comparable`.
public class GenericBridges {
    interface Source<T> {
        T next();
    }

    static class Counter implements Source<Integer> {
        int n;
        public Integer next() { return n++; }
    }

    static class Words implements Source<String> {
        String[] words = {"a", "bb", "ccc"};
        int i;
        public String next() { return words[i++ % words.length]; }
    }

    interface Sink<T> {
        void put(T t);
    }

    static class Total implements Sink<Integer> {
        int sum;
        public void put(Integer x) { sum += x; }
    }

    enum Size { SMALL, MEDIUM, LARGE }

    static class Point implements Comparable<Point> {
        final int x, y;
        Point(int x, int y) { this.x = x; this.y = y; }
        public int compareTo(Point o) { return x != o.x ? Integer.compare(x, o.x) : Integer.compare(y, o.y); }
        public String toString() { return "(" + x + "," + y + ")"; }
    }

    static <T extends Comparable<T>> T max(T a, T b) {
        return a.compareTo(b) >= 0 ? a : b;
    }

    static <T> void drain(Source<T> source, Sink<T> sink, int n) {
        for (int i = 0; i < n; i++) sink.put(source.next());
    }

    static <T> T pick(Source<T> s, int skip) {
        for (int i = 0; i < skip; i++) s.next();
        return s.next();
    }

    public static void main(String[] args) {
        Counter c = new Counter();
        Total t = new Total();
        drain(c, t, 5);
        System.out.println(t.sum + " " + c.next());
        Source<String> w = new Words();
        System.out.println(pick(w, 1) + " " + pick(new Words(), 4).length());
        System.out.println(max(3, 7) + " " + max("pear", "apple") + " " + max(Size.LARGE, Size.SMALL));
        System.out.println(max(new Point(1, 5), new Point(1, 2)) + " " + max(2.5, -1.0) + " " + max('a', 'z'));
        System.out.println(Size.SMALL.compareTo(Size.LARGE) + " " + new Point(0, 0).compareTo(new Point(0, 1)));
        Comparable<String> k = "b";
        System.out.println(k.compareTo("a"));
    }
}
