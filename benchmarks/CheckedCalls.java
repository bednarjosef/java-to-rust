// Small methods that check their arguments and throw, as libraries' validation helpers do: the
// Minecraft server's `SimpleBitStorage.get` calls Apache Commons' `Validate.inclusiveBetween` for
// every block it reads, whose check is two compares and whose throw formats a message. Packed
// values are read from and written to a bit storage, each access checked first.
public class CheckedCalls {
    static final class Storage {
        final long[] data;
        final int bits;
        final int size;
        final int valuesPerLong;
        final long mask;

        Storage(int bits, int size) {
            this.bits = bits;
            this.size = size;
            this.valuesPerLong = 64 / bits;
            this.mask = (1L << bits) - 1;
            this.data = new long[(size + valuesPerLong - 1) / valuesPerLong];
        }

        int get(int index) {
            inclusiveBetween(0, size - 1, index);
            int cell = index / valuesPerLong;
            int shift = (index - cell * valuesPerLong) * bits;
            return (int) (data[cell] >> shift & mask);
        }

        void set(int index, int value) {
            inclusiveBetween(0, size - 1, index);
            inclusiveBetween(0, mask, value);
            int cell = index / valuesPerLong;
            int shift = (index - cell * valuesPerLong) * bits;
            data[cell] = data[cell] & ~(mask << shift) | ((long) value & mask) << shift;
        }
    }

    static void inclusiveBetween(long start, long end, long value) {
        if (value < start || value > end) {
            throw new IllegalArgumentException(
                String.format("The value %s is not in the specified inclusive range of %s to %s", value, start, end));
        }
    }

    static long run(Storage storage, int rounds) {
        long sum = 0;
        for (int r = 0; r < rounds; r++) {
            for (int i = 0; i < storage.size; i++) {
                storage.set(i, (i * 31 + r) & 15);
            }
            for (int i = 0; i < storage.size; i++) {
                sum += storage.get(i);
            }
        }
        return sum;
    }

    public static void main(String[] args) {
        Storage storage = new Storage(5, 4096);
        long check = 0;
        for (int warm = 0; warm < 3; warm++) {
            check += run(storage, 2000);
        }
        long start = System.nanoTime();
        long result = run(storage, 20000);
        long elapsed = (System.nanoTime() - start) / 1000000;
        try {
            storage.get(storage.size);
        } catch (IllegalArgumentException e) {
            System.out.println(e.getMessage());
        }
        System.out.println("checked calls result " + result + " " + check);
        System.err.println("kernel-ms " + elapsed);
    }
}
