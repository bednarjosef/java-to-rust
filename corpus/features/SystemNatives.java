public class SystemNatives {
    static class Point {
        int x;
        int y;

        Point(int x, int y) {
            this.x = x;
            this.y = y;
        }

        void move(int dx) {
            x += dx;
        }

        boolean sameHash() {
            return hashCode() == System.identityHashCode(this);
        }
    }

    static long work(int n) {
        long s = 0;
        for (int i = 0; i < n; i++) {
            s += i % 7;
        }
        return s;
    }

    public static void main(String[] args) {
        // Clocks: only their properties are the same on every run.
        long start = System.nanoTime();
        long millis = System.currentTimeMillis();
        System.out.println(work(100000));
        long elapsed = System.nanoTime() - start;
        System.out.println("monotonic " + (elapsed >= 0) + ", after 2020 " + (millis > 1577836800000L));
        System.out.println("processors " + (Runtime.getRuntime().availableProcessors() >= 1));

        String nl = System.lineSeparator();
        System.out.println("separator " + nl.length() + " " + nl.equals("\n") + " "
            + System.getProperty("line.separator").equals(nl));
        System.out.print("two" + nl + "lines" + nl);

        // Identity hashes: the values differ from run to run, but they stay the same for an
        // object's life, and Object.hashCode is the identity hash when no class overrides it.
        Point p = new Point(1, 2);
        Point q = new Point(1, 2);
        int h = System.identityHashCode(p);
        p.move(5);
        System.out.println("stable " + (System.identityHashCode(p) == h) + " " + (p.hashCode() == h) + " "
            + p.sameHash());
        System.out.println("distinct " + (System.identityHashCode(q) != h) + " nonnegative " + (h >= 0));

        // Standard error is not compared, but what is printed before it must come out first.
        System.err.println("to standard error " + p.x);
        System.err.print("no newline");
        System.err.println();
        System.out.write(72);
        System.out.write(105);
        System.out.write(10);
        System.out.write(256 + 33);
        System.out.flush();
        System.out.println();
        char[] greeting = {'h', 'e', 'l', 'l', 'o', ' ', 'é', '世'};
        System.out.print(greeting);
        System.out.println(greeting);
        System.out.println(Math.E + " " + Math.PI + " " + StrictMath.PI);
    }
}
