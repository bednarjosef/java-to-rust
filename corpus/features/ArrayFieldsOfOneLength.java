// Array fields whose every array has one length: final fields made with a constant size, read in
// loops that count between constants, at constant indexes, through other objects, through a
// variable that copies the field, and with guards on the length; a static final literal; and
// accesses past the end, which still throw ArrayIndexOutOfBoundsException.
public class ArrayFieldsOfOneLength {
    static final int[] STEPS = {1, 2, 4, 8};

    // A cache of the last four lookups, kept most recent first.
    static class Cache {
        private final long[] keys = new long[4];
        private final String[] values = new String[4];
        private final int[] hits;
        private int misses;

        Cache() {
            this.hits = new int[4];
            for (int i = 0; i < 4; i++) {
                this.keys[i] = -1;
            }
        }

        String get(long key) {
            for (int i = 0; i < 4; i++) {
                if (this.keys[i] == key && this.values[i] != null) {
                    this.hits[i]++;
                    return this.values[i];
                }
            }
            this.misses++;
            String value = "v" + key * key;
            this.store(key, value);
            return value;
        }

        private void store(long key, String value) {
            for (int i = 3; i > 0; i--) {
                this.keys[i] = this.keys[i - 1];
                this.values[i] = this.values[i - 1];
                this.hits[i] = this.hits[i - 1];
            }
            this.keys[0] = key;
            this.values[0] = value;
            this.hits[0] = 0;
        }

        long sumKeys() {
            long[] copy = this.keys;
            long sum = 0;
            for (int i = 0; i < copy.length; i++) {
                sum += copy[i];
            }
            return sum;
        }

        int hitsAt(int slot) {
            // A guard on the field's length proves the index, as a constant bound does.
            if (slot >= 0 && slot < this.hits.length) {
                return this.hits[slot];
            }
            return -1;
        }

        long keyFor(int hash) {
            return this.keys[hash & 3];
        }

        int hitsPast() {
            int sum = 0;
            for (int i = 0; i <= 4; i++) {
                sum += this.hits[i];
            }
            return sum;
        }
    }

    // A final field that two constructors make with different lengths: its accesses keep their
    // checks.
    static class Uneven {
        final int[] cells;

        Uneven() {
            this.cells = new int[2];
        }

        Uneven(boolean big) {
            this.cells = new int[big ? 8 : 3];
        }

        int last(int i) {
            return this.cells[i];
        }
    }

    static long otherKeys(Cache other) {
        long sum = 0;
        for (int i = 0; i < 4; i++) {
            sum += other.keys[i] * STEPS[i];
        }
        return sum + other.keys[3] - other.keys[0];
    }

    static int stepSum(int n) {
        int sum = 0;
        for (int i = 0; i < STEPS.length; i++) {
            sum += STEPS[i] * n;
        }
        for (int i = 1; i <= 3; i += 2) {
            sum += STEPS[i - 1] + STEPS[i];
        }
        return sum;
    }

    public static void main(String[] args) {
        Cache cache = new Cache();
        long[] keys = {3, 5, 3, 7, 9, 11, 3, 5, 13, 13};
        StringBuilder seen = new StringBuilder();
        for (long k : keys) {
            seen.append(cache.get(k)).append(' ');
        }
        System.out.println(seen.toString().trim());
        System.out.println("misses " + cache.misses + " sum " + cache.sumKeys() + " other " + otherKeys(cache));
        System.out.println("hits " + cache.hitsAt(0) + " " + cache.hitsAt(3) + " " + cache.hitsAt(4) + " " + cache.hitsAt(-1));
        System.out.println("steps " + stepSum(3) + " masked " + cache.keyFor(-7) + " " + cache.keyFor(1 << 30));
        try {
            System.out.println(cache.hitsPast());
        } catch (ArrayIndexOutOfBoundsException e) {
            System.out.println("past the end: " + e.getMessage());
        }
        Uneven small = new Uneven();
        Uneven big = new Uneven(true);
        System.out.println("uneven " + big.last(7));
        try {
            System.out.println(small.last(5));
        } catch (ArrayIndexOutOfBoundsException e) {
            System.out.println("uneven: " + e.getMessage());
        }
        int index = args.length + 6;
        try {
            System.out.println(STEPS[index]);
        } catch (ArrayIndexOutOfBoundsException e) {
            System.out.println("steps: " + e.getMessage());
        }
    }
}
