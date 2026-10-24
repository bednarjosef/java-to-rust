import java.util.function.Function;

public class StackObjects {
    static class Buffer {
        int[] items = new int[4];
        int count;
        Buffer add(int x) {
            if (count == items.length) {
                int[] more = new int[count * 2];
                System.arraycopy(items, 0, more, 0, count);
                items = more;
            }
            items[count++] = x;
            return this;
        }
        int sum() {
            int s = 0;
            for (int i = 0; i < count; i++) s += items[i];
            return s;
        }
    }

    static class Point {
        final int x, y;
        Point(int x, int y) { this.x = x; this.y = y; }
        Point plus(Point o) { return new Point(x + o.x, y + o.y); }
    }

    static class Counter {
        int n;
        Counter() { reset(this); }
        static void reset(Counter c) { c.n = 0; }
    }

    static Point kept;

    static int apply(Function<Integer, Integer> f, int v) { return f.apply(v); }

    public static void main(String[] args) {
        int total = 0;
        for (int i = 0; i < 1000; i++) {
            Buffer b = new Buffer();
            b.add(i).add(i + 1).add(2).add(3).add(4);
            total += b.sum();
        }
        System.out.println(total);
        Point p = new Point(1, 2);
        Point q = new Point(3, 4);
        Point r = p.plus(q);
        System.out.println(r.x + " " + r.y);
        Point last = null;
        for (int i = 0; i < 3; i++) {
            Point t = new Point(i, i);
            if (i == 1) kept = t;
            last = new Point(i * 10, 0);
        }
        System.out.println(kept.x + " " + last.x);
        Counter c = new Counter();
        c.n += 5;
        System.out.println(c.n);
        int k = 7;
        int sum = 0;
        for (int i = 0; i < 10; i++) {
            final int j = i;
            sum += apply(v -> v + j + k, i);
        }
        System.out.println(sum);
        Object o1 = new Object();
        Object o2 = new Object();
        System.out.println(o1 == o2);
        Buffer x = new Buffer();
        Buffer y = x.add(1);
        System.out.println(x == y);
    }
}
