public class Sieve {
    static int countPrimes(boolean[] composite) {
        int count = 0;
        for (int i = 2; i < composite.length; i++) {
            if (!composite[i]) {
                count++;
            }
        }
        return count;
    }

    public static void main(String[] args) {
        int limit = 100;
        boolean[] composite = new boolean[limit + 1];
        for (int i = 2; i * i <= limit; i++) {
            if (!composite[i]) {
                for (int j = i * i; j <= limit; j += i) {
                    composite[j] = true;
                }
            }
        }
        System.out.println("primes below " + limit + ": " + countPrimes(composite));

        int[] primes = new int[countPrimes(composite)];
        int n = 0;
        for (int i = 2; i <= limit; i++) {
            if (!composite[i]) {
                primes[n++] = i;
            }
        }
        // Sort a copy in descending order by negating, then restore the sign.
        int[] negated = new int[primes.length];
        for (int i = 0; i < primes.length; i++) {
            negated[i] = -primes[i];
        }
        java.util.Arrays.sort(negated);
        int[] largest = new int[5];
        for (int i = 0; i < largest.length; i++) {
            largest[i] = -negated[i];
        }
        System.out.println(java.util.Arrays.toString(largest));

        long checksum = 0;
        for (int p : primes) {
            checksum = checksum * 31 + p;
        }
        System.out.println("checksum " + checksum);
        System.out.println(primes[primes.length - 1] / primes[0]);
    }
}
