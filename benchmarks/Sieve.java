// Sieve of Eratosthenes over a boolean array, repeated for several limits: an integer loop with
// array writes, and a counting pass over the array.
public class Sieve {
    static int countPrimes(int limit) {
        boolean[] composite = new boolean[limit + 1];
        for (int i = 2; i * i <= limit; i++) {
            if (!composite[i]) {
                for (int j = i * i; j <= limit; j += i) {
                    composite[j] = true;
                }
            }
        }
        int count = 0;
        for (int i = 2; i <= limit; i++) {
            if (!composite[i]) {
                count++;
            }
        }
        return count;
    }

    public static void main(String[] args) {
        int limit = 100000000;
        long checksum = 0;
        for (int round = 0; round < 4; round++) {
            checksum = checksum * 31 + countPrimes(limit - round);
        }
        System.out.println("primes below " + limit + " checksum " + checksum);
    }
}
