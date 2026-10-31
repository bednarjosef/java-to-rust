// Each step of the loop runs a little of many parts of the library: formatting, a regular
// expression, a tree map and a hash map, big integers and decimals, number printing, a string
// builder, sorting and a deque. The hot code is spread over many functions far apart in the
// program's code, as in a server's tick, so the program measures how well the processor's
// instruction cache and instruction TLB hold it (docs/performance.md, "Hot code together").
import java.math.BigDecimal;
import java.math.BigInteger;
import java.math.RoundingMode;
import java.util.ArrayDeque;
import java.util.Arrays;
import java.util.HashMap;
import java.util.List;
import java.util.TreeMap;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

public class ManyLibraryPaths {
    static final String[] WORDS = {"alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta", "theta"};

    static long steps(int count, int from) {
        Pattern pattern = Pattern.compile("([a-z]+)-(\\d+)");
        TreeMap<String, Integer> tree = new TreeMap<>();
        HashMap<Integer, String> hash = new HashMap<>();
        ArrayDeque<Long> deque = new ArrayDeque<>();
        BigDecimal seven = new BigDecimal(7);
        long sum = 0;
        for (int i = from; i < from + count; i++) {
            String text = String.format("%s-%d", WORDS[i & 7], i % 1000);
            Matcher matcher = pattern.matcher(text);
            if (matcher.matches()) {
                sum += Integer.parseInt(matcher.group(2)) + matcher.group(1).length();
                tree.merge(matcher.group(1) + (i & 63), 1, Integer::sum);
            }
            if (tree.size() > 48) {
                sum += tree.pollFirstEntry().getValue();
            }
            hash.put(i & 1023, text);
            sum += hash.get((i * 7) & 1023) == null ? 0 : 1;
            sum += BigInteger.valueOf(i).pow(3).add(BigInteger.TEN).bitLength();
            sum += new BigDecimal(i).divide(seven, 10, RoundingMode.HALF_UP).unscaledValue().intValue() & 255;
            sum += Double.toString(i / 3.0).length() + Long.toHexString(i * 31L).hashCode();
            StringBuilder builder = new StringBuilder().append(i).append(':').append(text.toUpperCase());
            sum += builder.reverse().toString().hashCode();
            int[] values = {i, i ^ 5, i * 3, 7, i >> 2};
            Arrays.sort(values);
            sum += Arrays.binarySearch(values, 7);
            deque.addLast((long) i);
            if (deque.size() > 16) {
                sum += deque.pollFirst();
            }
            sum += String.join(",", List.of("a", text, "b")).split(",").length;
        }
        return sum;
    }

    public static void main(String[] args) {
        long warm = steps(20000, 0);
        long start = System.nanoTime();
        long sum = steps(200000, 20000);
        System.err.println("kernel-ms " + (System.nanoTime() - start) / 1000000);
        System.out.println(warm + " " + sum);
    }
}
