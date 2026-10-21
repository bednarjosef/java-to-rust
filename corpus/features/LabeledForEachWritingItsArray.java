// Labeled for-each loops over an array whose body writes the array's elements, left by a
// labeled break or continue from an inner loop.
public class LabeledForEachWritingItsArray {
    public static void main(String[] args) {
        byte[] a = new byte[4];
        int seen = 0;
        outer:
        for (byte x : a) {
            a[seen % a.length] += 3;
            for (int k = 0; k < 3; k++) {
                if (k == seen) {
                    seen++;
                    continue outer;
                }
            }
            break outer;
        }
        System.out.println(seen + " " + a[0] + " " + a[1] + " " + a[2] + " " + a[3]);

        long[] b = {5L, 6L, 7L};
        long sum = 0;
        walk:
        for (long x : b) {
            b[b.length - 1] = x * 10;
            int j = 0;
            while (true) {
                if (j++ > 1 && x == 6L) {
                    break walk;
                }
                if (j > 2) {
                    break;
                }
            }
            sum += x;
        }
        System.out.println(sum + " " + b[2]);
    }
}
