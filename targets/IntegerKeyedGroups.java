// Features: HashMap with Integer keys and List values, computeIfAbsent, getOrDefault,
// HashSet of Integers, Set operations (retainAll, addAll), Integer boxing and unboxing,
// Integer.compare, Integer.MAX_VALUE, integer division and remainder.
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

public class IntegerKeyedGroups {
    static Map<Integer, List<String>> groupByDigitSum(String[] names, int[] numbers) {
        Map<Integer, List<String>> groups = new HashMap<>();
        for (int i = 0; i < numbers.length; i++) {
            int sum = digitSum(numbers[i]);
            groups.computeIfAbsent(sum, k -> new ArrayList<>()).add(names[i]);
        }
        return groups;
    }

    static int digitSum(int n) {
        int sum = 0;
        while (n > 0) {
            sum += n % 10;
            n /= 10;
        }
        return sum;
    }

    static Set<Integer> primesUpTo(int limit) {
        Set<Integer> primes = new HashSet<>();
        for (int n = 2; n <= limit; n++) {
            boolean prime = true;
            for (int d = 2; d * d <= n; d++) {
                if (n % d == 0) {
                    prime = false;
                    break;
                }
            }
            if (prime) {
                primes.add(n);
            }
        }
        return primes;
    }

    public static void main(String[] args) {
        String[] names = {"ann", "bob", "cid", "dee", "eve", "fay", "gus"};
        int[] numbers = {12, 31, 7, 400, 22, 9, 1001};
        Map<Integer, List<String>> groups = groupByDigitSum(names, numbers);
        for (int key = 0; key <= 10; key++) {
            List<String> members = groups.get(key);
            if (members != null) {
                System.out.println("digit sum " + key + ": " + members);
            }
        }
        System.out.println("digit sum 99 count: " + groups.getOrDefault(99, new ArrayList<>()).size());

        Set<Integer> primes = primesUpTo(30);
        System.out.println("primes: " + primes.size() + " contains 13? " + primes.contains(13));
        Set<Integer> odds = new HashSet<>();
        for (int i = 1; i <= 30; i += 2) {
            odds.add(i);
        }
        Set<Integer> oddPrimes = new HashSet<>(primes);
        oddPrimes.retainAll(odds);
        System.out.println("odd primes: " + oddPrimes.size());
        Set<Integer> union = new HashSet<>(primes);
        union.addAll(List.of(1, 4, 6, 8, 9, 10));
        System.out.println("union size: " + union.size());
        union.removeAll(primes);
        System.out.println("non-primes: " + union.size());
        System.out.println("removed 1? " + union.remove(1) + ", again? " + union.remove(1));

        Integer boxedA = 1000;
        Integer boxedB = 1000;
        System.out.println("equals: " + boxedA.equals(boxedB) + ", compare: " + Integer.compare(boxedA, 999));
        int unboxed = boxedA + boxedB;
        System.out.println("sum: " + unboxed);
        int overflow = Integer.MAX_VALUE;
        overflow += 1;
        System.out.println("overflow: " + overflow);
        System.out.println("division: " + (-7 / 2) + " remainder: " + (-7 % 2));
        System.out.println("hash sizes: " + groups.size() + " " + primes.isEmpty());
    }
}
