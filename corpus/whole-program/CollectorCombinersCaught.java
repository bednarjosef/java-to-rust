import java.util.AbstractSet;
import java.util.ArrayList;
import java.util.Iterator;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import java.util.function.BinaryOperator;
import java.util.stream.Collector;
import java.util.stream.Collectors;

// The combiners of collectors, which merge two partial results of a parallel stream, called
// directly where a catch prints what it catches: the JDK's combiners are lambdas, some of which
// use the collector's arguments, with a loop over the entries of a map. A set whose iterator may
// be null makes the loop's iterator one that may be null, whose message the catch may print.
public class CollectorCombinersCaught {
    static class Given<E> extends AbstractSet<E> {
        Iterator<E> iterator;

        public Iterator<E> iterator() {
            return iterator;
        }

        public int size() {
            return 0;
        }
    }

    static <A, R> R combine(Collector<? super String, A, R> collector, List<String> left, List<String> right) {
        A a = collector.supplier().get();
        A b = collector.supplier().get();
        left.forEach(s -> collector.accumulator().accept(a, s));
        right.forEach(s -> collector.accumulator().accept(b, s));
        BinaryOperator<A> combiner = collector.combiner();
        return collector.finisher().apply(combiner.apply(a, b));
    }

    public static void main(String[] args) {
        List<String> left = List.of("a", "bb", "ccc");
        List<String> right = List.of("dd", "e", "fff");
        try {
            Given<String> given = new Given<>();
            given.iterator = left.iterator();
            for (String s : given) {
                System.out.println(s);
            }
            Map<Integer, Integer> sums = combine(Collectors.toMap(String::length, s -> 1, Integer::sum), left, right);
            System.out.println(new TreeMap<>(sums));
            Map<String, Integer> unique = combine(Collectors.toMap(s -> s, String::length), left, right);
            System.out.println(new TreeMap<>(unique));
            Map<Integer, List<String>> groups = combine(Collectors.groupingBy(String::length), left, right);
            System.out.println(new TreeMap<>(groups));
            Map<Integer, Long> counts = combine(Collectors.groupingBy(String::length, HashMap::new, Collectors.counting()), left, right);
            System.out.println(new TreeMap<>(counts));
            Map<Boolean, List<String>> parts = combine(Collectors.partitioningBy(s -> s.length() > 1), left, right);
            System.out.println(parts);
            List<String> all = combine(Collectors.toCollection(ArrayList::new), left, right);
            System.out.println(all);
            System.out.println(combine(Collectors.joining(","), left, right));
            Map<Integer, Integer> clash = combine(Collectors.toMap(String::length, s -> 1), left, right);
            System.out.println(clash);
        } catch (Exception e) {
            System.out.println(e.getClass().getName() + ": " + e.getMessage());
        }
    }
}
