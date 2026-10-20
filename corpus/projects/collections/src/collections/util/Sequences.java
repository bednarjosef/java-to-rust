package collections.util;

// Algorithms over any sequence: the ones that compare elements take a type variable bounded by
// `Comparable`.
public final class Sequences {
    private Sequences() {}

    public static <T extends Comparable<T>> T max(Sequence<T> s) {
        T best = s.get(0);
        for (int i = 1; i < s.size(); i++) {
            if (s.get(i).compareTo(best) > 0) {
                best = s.get(i);
            }
        }
        return best;
    }

    public static <T extends Comparable<T>> void sort(ArraySequence<T> s) {
        for (int i = 1; i < s.size(); i++) {
            T x = s.get(i);
            int j = i - 1;
            while (j >= 0 && s.get(j).compareTo(x) > 0) {
                s.set(j + 1, s.get(j));
                j--;
            }
            s.set(j + 1, x);
        }
    }

    public static <T> int count(Sequence<T> s, T x) {
        int n = 0;
        for (int i = 0; i < s.size(); i++) {
            if (s.get(i).equals(x)) {
                n++;
            }
        }
        return n;
    }

    public static String show(Sequence<?> s) {
        StringBuilder sb = new StringBuilder("[");
        for (int i = 0; i < s.size(); i++) {
            if (i > 0) {
                sb.append(", ");
            }
            sb.append(s.get(i));
        }
        return sb.append("]").toString();
    }
}
