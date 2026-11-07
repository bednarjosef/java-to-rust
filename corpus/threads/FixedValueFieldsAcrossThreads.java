// Objects that threads share, whose final fields are numbers, booleans, chars and enums, read in
// hot loops by other threads after the objects are published: through Thread.start, a volatile
// field, a field written and read under a lock, and a final static. The fields are read with plain loads once the
// constructor has written them; the constructor reads them back itself, and so does a method it
// calls. One class hands `this` to a thread it starts before writing its fields, which keeps them
// atomic.
public class FixedValueFieldsAcrossThreads {
    enum Shape { SQUARE, CIRCLE, TRIANGLE }

    static class Spec {
        final int width;
        final long seed;
        final double scale;
        final boolean mirrored;
        final char mark;
        final short depth;
        final byte bias;
        final float weight;
        final Shape shape;
        final int area;
        int uses;

        Spec(int width, long seed, double scale, boolean mirrored, Shape shape) {
            this.width = width;
            this.seed = seed;
            this.scale = scale;
            this.mirrored = mirrored;
            this.mark = (char) ('a' + width % 26);
            this.depth = (short) (width * 3);
            this.bias = (byte) (width - 7);
            this.weight = (float) scale / 2;
            this.shape = shape;
            this.area = measure();
            this.uses = this.width + this.depth;
        }

        int measure() {
            switch (shape) {
                case SQUARE: return width * width;
                case CIRCLE: return (int) (scale * width * width);
                default: return width * width / 2;
            }
        }

        long step(long x) {
            long y = x * seed + width;
            if (mirrored) {
                y = -y;
            }
            return y + mark + depth + bias + area + (shape == Shape.CIRCLE ? 1 : 0);
        }

        synchronized void use() {
            uses++;
        }
    }

    // Hands `this` to a thread it starts before writing its fields, so they stay atomic.
    static class Leaky {
        final Thread pinger;
        final int value;
        final Shape shape;

        Leaky(int value, Shape shape) {
            pinger = new Thread(this::ping);
            pinger.start();
            this.value = value;
            this.shape = shape;
        }

        void ping() {
            System.out.println("ping");
        }
    }

    static final Spec CONSTANT = new Spec(5, 3L, 0.25, false, Shape.TRIANGLE);
    static volatile Spec published;
    static final Object lock = new Object();
    static Spec handed;

    static long run(Spec s, int n) {
        long total = 0;
        double sum = 0;
        for (int i = 0; i < n; i++) {
            total += s.step(i) + s.width + (s.mirrored ? 1 : 2);
            sum += s.scale * s.weight + s.width;
            if (s.shape == Shape.SQUARE) {
                total ^= s.seed;
            }
        }
        return total + (long) sum;
    }

    public static void main(String[] args) throws InterruptedException {
        Spec first = new Spec(9, 1_000_003L, 1.5, true, Shape.CIRCLE);
        long[] totals = new long[4];
        Thread[] threads = new Thread[4];
        // Published through Thread.start.
        threads[0] = new Thread(() -> totals[0] = run(first, 200_000));
        threads[0].start();
        // Published through a volatile field.
        threads[1] = new Thread(() -> {
            Spec s;
            while ((s = published) == null) {
                Thread.onSpinWait();
            }
            totals[1] = run(s, 200_000);
            s.use();
        });
        threads[1].start();
        published = new Spec(12, 77L, 0.5, false, Shape.SQUARE);
        // Published under a lock.
        threads[2] = new Thread(() -> {
            Spec s = null;
            while (s == null) {
                synchronized (lock) {
                    s = handed;
                }
            }
            totals[2] = run(s, 200_000);
            s.use();
        });
        threads[2].start();
        synchronized (lock) {
            handed = new Spec(20, -5L, 2.0, true, Shape.TRIANGLE);
        }
        // Published through a final static.
        threads[3] = new Thread(() -> totals[3] = run(CONSTANT, 200_000));
        threads[3].start();
        Leaky leaky = new Leaky(41, Shape.CIRCLE);
        leaky.pinger.join();
        long leaked = 0;
        for (int i = 0; i < 1000; i++) {
            leaked += leaky.value + leaky.shape.ordinal();
        }
        for (Thread t : threads) {
            t.join();
        }
        for (int t = 0; t < totals.length; t++) {
            System.out.println("thread " + t + ": " + totals[t]);
        }
        System.out.println("leaked " + leaked);
        for (Spec s : new Spec[] {first, published, CONSTANT}) {
            System.out.println(s.width + " " + s.seed + " " + s.scale + " " + s.mirrored + " " + s.mark + " "
                + s.depth + " " + s.bias + " " + s.weight + " " + s.shape + " " + s.area + " " + s.uses);
        }
    }
}
