// Threads read and write the same elements of shared arrays at once, without synchronization:
// a `long[]` and a `double[]` whose writers store values that differ in both halves, a
// `String[]` whose elements are swapped between two strings, and an array of objects that a
// writer publishes. Each element access is atomic, so a reader only ever sees a value some
// thread stored; the final values are decided by `join`.
public class SharedArrayRaces {
    static final int ROUNDS = 20000;
    static long[] longs = new long[4];
    static double[] doubles = new double[4];
    static String[] names = {"left", "left", "left", "left"};
    static Box[] boxes = new Box[8];

    static class Box {
        final int value;

        Box(int value) {
            this.value = value;
        }
    }

    public static void main(String[] args) throws InterruptedException {
        final long a = 0x0123456789abcdefL;
        final long b = 0xfedcba9876543210L;
        Thread longWriter = new Thread(() -> {
            for (int i = 0; i < ROUNDS; i++) {
                longs[i & 3] = (i & 1) == 0 ? a : b;
                doubles[i & 3] = (i & 1) == 0 ? 1.0e300 : -2.5e-300;
            }
        });
        Thread nameWriter = new Thread(() -> {
            for (int i = 0; i < ROUNDS; i++) {
                names[i & 3] = (i & 1) == 0 ? "right" : "left";
            }
        });
        Thread boxWriter = new Thread(() -> {
            for (int i = 0; i < boxes.length; i++) {
                boxes[i] = new Box(i * 10);
            }
        });
        longWriter.start();
        nameWriter.start();
        boxWriter.start();
        long torn = 0;
        long odd = 0;
        for (int i = 0; i < ROUNDS; i++) {
            long v = longs[i & 3];
            if (v != 0 && v != a && v != b) {
                torn++;
            }
            double d = doubles[i & 3];
            if (d != 0 && d != 1.0e300 && d != -2.5e-300) {
                torn++;
            }
            String s = names[i & 3];
            if (!s.equals("left") && !s.equals("right")) {
                odd++;
            }
            Box box = boxes[i & 7];
            if (box != null && box.value != (i & 7) * 10) {
                odd++;
            }
        }
        longWriter.join();
        nameWriter.join();
        boxWriter.join();
        System.out.println("torn values: " + torn);
        System.out.println("odd values: " + odd);
        // The last round, ROUNDS - 1, is odd, and wrote `b` to element 3; round ROUNDS - 2 wrote
        // `a` to element 2.
        System.out.println(Long.toHexString(longs[3]) + " " + Long.toHexString(longs[2]));
        System.out.println(doubles[3] + " " + doubles[2]);
        System.out.println(String.join(",", names));
        int sum = 0;
        for (Box box : boxes) {
            sum += box.value;
        }
        System.out.println("boxes: " + sum);
    }
}
