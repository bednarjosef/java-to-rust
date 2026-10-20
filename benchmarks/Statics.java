// Static fields and methods: a xorshift generator whose state lives in a static field, a static
// histogram updated in a hot loop, and a recursive static method.
public class Statics {
    static long state = 88172645463325252L;
    static int calls;
    static int[] histogram = new int[16];

    static long next() {
        state ^= state << 13;
        state ^= state >>> 7;
        state ^= state << 17;
        calls++;
        return state;
    }

    static void tally(int bucket) {
        histogram[bucket]++;
    }

    static int fib(int n) {
        return n < 2 ? n : fib(n - 1) + fib(n - 2);
    }

    public static void main(String[] args) {
        for (int i = 0; i < 1000000000; i++) {
            long r = next();
            tally((int) (r & 15));
        }
        long weighted = 0;
        for (int b = 0; b < histogram.length; b++) {
            weighted += (long) histogram[b] * (b + 1);
        }
        System.out.println("calls " + calls + " state " + state + " weighted " + weighted);
        System.out.println("fib(36) = " + fib(36));
    }
}
