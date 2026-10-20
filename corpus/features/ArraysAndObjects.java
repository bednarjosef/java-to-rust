class Stats {
    int count;
    long total;

    void addAll(int[] values) {
        for (int i = 0; i < values.length; i++) {
            total += values[i];
            count++;
        }
    }

    int[] histogram(int[] values, int buckets) {
        int[] h = new int[buckets];
        for (int v : values) {
            h[v % buckets]++;
        }
        return h;
    }

    public String toString() {
        return "Stats(" + count + ", " + total + ")";
    }
}

class Point {
    final int x;
    final int y;

    Point(int x, int y) {
        this.x = x;
        this.y = y;
    }

    int dot(Point o) {
        return x * o.x + y * o.y;
    }
}

public class ArraysAndObjects {
    static int[] squares(int n) {
        int[] out = new int[n];
        for (int i = 0; i < n; i++) {
            out[i] = i * i;
        }
        return out;
    }

    public static void main(String[] args) {
        Stats s = new Stats();
        int[] v = squares(6);
        s.addAll(v);
        s.addAll(new int[] {7, 8});
        System.out.println(s);
        int[] h = s.histogram(v, 3);
        System.out.println(java.util.Arrays.toString(h));
        Point[] ps = {new Point(1, 2), new Point(3, 4)};
        int sum = 0;
        for (Point p : ps) {
            sum += p.dot(ps[0]);
        }
        System.out.println(sum + " " + ps.length);
        Stats shared = s;
        shared.addAll(h);
        System.out.println(s + " " + shared);
    }
}
