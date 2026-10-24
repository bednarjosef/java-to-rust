import java.util.Arrays;

// The bulk operations on arrays that threads share: `System.arraycopy` between shared arrays,
// within one (overlapping), and between a shared array and a local one, run by workers on their
// own parts of one array at once; `Arrays.fill`, `sort`, `copyOf`, `equals`, `hashCode` and
// `binarySearch`; `clone()`; and the library calls that read or write a range of a shared array
// (`new String(chars, offset, count)`, `StringBuilder.append(chars, offset, len)`,
// `String.getChars`, `new String(bytes, offset, length)`).
public class SharedArrayCopies {
    static int[] source = new int[40];
    static int[] target = new int[40];
    static long[] longs = new long[12];
    static char[] chars = new char[24];
    static byte[] bytes = new byte[16];
    static String[] words = new String[6];
    static int[][] grid = new int[4][5];

    public static void main(String[] args) throws InterruptedException {
        for (int i = 0; i < source.length; i++) {
            source[i] = i * i - 7 * i;
        }
        Thread[] workers = new Thread[4];
        for (int w = 0; w < workers.length; w++) {
            final int part = w;
            workers[w] = new Thread(() -> {
                // Each worker copies its tenth of `source` into `target`, then shifts its own part
                // of `target` by one within the array.
                System.arraycopy(source, part * 10, target, part * 10, 10);
                System.arraycopy(target, part * 10, target, part * 10 + 1, 9);
                int[] local = new int[5];
                for (int k = 0; k < local.length; k++) {
                    local[k] = part * 100 + k;
                }
                System.arraycopy(local, 0, grid[part], 0, 5);
                longs[part * 3] = (long) part << 40;
                longs[part * 3 + 1] = -part;
                longs[part * 3 + 2] = part * 1000003L;
                words[part] = "w" + (3 - part);
                String text = "part" + part;
                text.getChars(0, 5, chars, part * 6);
                chars[part * 6 + 5] = '|';
                bytes[part * 4] = (byte) ('a' + part);
                bytes[part * 4 + 1] = (byte) ('A' + part);
                bytes[part * 4 + 2] = (byte) ('0' + part);
                bytes[part * 4 + 3] = (byte) ' ';
            });
            workers[w].start();
        }
        for (Thread t : workers) {
            t.join();
        }
        System.out.println(Arrays.toString(target));
        System.out.println(Arrays.deepToString(grid));
        System.out.println(new String(chars));
        System.out.println(new String(chars, 6, 11));
        StringBuilder sb = new StringBuilder();
        sb.append(chars, 12, 5).append('/').append(chars, 0, 4);
        System.out.println(sb);
        System.out.println(new String(bytes, 4, 8));
        int[] back = new int[8];
        System.arraycopy(target, 30, back, 0, 8);
        System.out.println(Arrays.toString(back));
        words[4] = "w9";
        words[5] = "w5";
        String[] sortedWords = words.clone();
        Arrays.sort(sortedWords);
        System.out.println(Arrays.toString(words) + " " + Arrays.toString(sortedWords));
        long[] copy = Arrays.copyOf(longs, 14);
        System.out.println(Arrays.equals(longs, Arrays.copyOf(copy, 12)) + " " + Arrays.hashCode(longs));
        Arrays.sort(longs);
        System.out.println(Arrays.toString(longs));
        System.out.println(Arrays.binarySearch(longs, 2000006L) + " " + Arrays.binarySearch(longs, 5L));
        Arrays.fill(longs, 2, 6, 99L);
        System.out.println(Arrays.toString(longs) + " " + Arrays.toString(copy));
        int[] twice = target.clone();
        System.arraycopy(twice, 0, twice, 3, 20);
        System.out.println(Arrays.toString(twice));
    }
}
