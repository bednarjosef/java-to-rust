import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;

public class Pairs {
    record Pair<A, B>(A first, B second) {
        <C> Pair<A, C> withSecond(C second) {
            return new Pair<>(first, second);
        }
    }

    static <T extends Comparable<? super T>> T largest(List<? extends T> items) {
        T best = null;
        for (T item : items) {
            if (best == null || item.compareTo(best) > 0) {
                best = item;
            }
        }
        return best;
    }

    public static void main(String[] args) {
        List<Pair<String, Integer>> pairs = new ArrayList<>();
        pairs.add(new Pair<>("b", 2));
        pairs.add(new Pair<>("a", 1));
        pairs.sort(Comparator.comparing(Pair::first));
        System.out.println(pairs + " " + largest(List.of(3, 9, 4)) + " " + pairs.get(0).withSecond(2.5));
    }
}
