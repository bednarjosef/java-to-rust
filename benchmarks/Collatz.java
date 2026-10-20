// Longest Collatz chain below a limit: long arithmetic in a data-dependent loop, called for every
// starting number.
public class Collatz {
    static int chainLength(long n) {
        int steps = 1;
        while (n != 1) {
            if (n % 2 == 0) {
                n = n / 2;
            } else {
                n = 3 * n + 1;
            }
            steps++;
        }
        return steps;
    }

    public static void main(String[] args) {
        int limit = 6000000;
        int best = 1;
        int bestLength = 1;
        long total = 0;
        for (int start = 1; start < limit; start++) {
            int length = chainLength(start);
            total += length;
            if (length > bestLength) {
                best = start;
                bestLength = length;
            }
        }
        System.out.println("longest chain under " + limit + " starts at " + best + " with " + bestLength + " steps");
        System.out.println("total steps " + total);
    }
}
