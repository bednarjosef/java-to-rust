// Many large arrays alive at once, of sizes that are whole multiples of 64 KB of elements (128 KB
// of longs, 64 KB of ints, 256 KB of doubles), as the buffers of lists and maps that grew by
// doubling are: what it measures is the memory they take (`scripts/benchmark.py` reports the peak).
public class LargeArrays {
    public static void main(String[] args) {
        long[][] longs = new long[300][];
        int[][] ints = new int[300][];
        double[][] doubles = new double[100][];
        for (int i = 0; i < longs.length; i++) {
            longs[i] = new long[16384];
            ints[i] = new int[16384];
            for (int j = 0; j < 16384; j += 61) {
                longs[i][j] = (long) i * j;
                ints[i][j] = i ^ j;
            }
        }
        for (int i = 0; i < doubles.length; i++) {
            doubles[i] = new double[32768];
            for (int j = 0; j < 32768; j += 97) {
                doubles[i][j] = i + j * 0.5;
            }
        }
        long sum = 0;
        double total = 0;
        for (int i = 0; i < longs.length; i++) {
            for (int j = 0; j < 16384; j += 61) {
                sum += longs[i][j] + ints[i][j];
            }
        }
        for (double[] d : doubles) {
            for (int j = 0; j < d.length; j += 97) {
                total += d[j];
            }
        }
        System.out.println(sum + " " + total);
    }
}
