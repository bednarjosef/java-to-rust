// A pattern under a negation as the condition of `?:`, as decompilers write `equals`:
// `!(o instanceof T t) ? false : ..t..`, whose variable is in scope in the false branch.
import java.util.Objects;

public class NegatedPatternConditionals {
    static final class Point {
        final int x;
        final int y;

        Point(int x, int y) {
            this.x = x;
            this.y = y;
        }

        @Override
        public boolean equals(Object o) {
            if (this == o) {
                return true;
            } else {
                return !(o instanceof Point p) ? false : this.x == p.x && this.y == p.y;
            }
        }

        @Override
        public int hashCode() {
            return 31 * x + y;
        }
    }

    static final class Pair<A, B> {
        final A first;
        final B second;

        Pair(A first, B second) {
            this.first = first;
            this.second = second;
        }

        @Override
        public boolean equals(Object obj) {
            return !(obj instanceof Pair<?, ?> other)
                ? false
                : Objects.equals(this.first, other.first) && Objects.equals(this.second, other.second);
        }

        @Override
        public int hashCode() {
            return Objects.hash(first, second);
        }
    }

    static int length(Object o) {
        return !(o instanceof String s && !s.isEmpty()) ? -1 : s.length();
    }

    public static void main(String[] args) {
        Point a = new Point(1, 2);
        System.out.println(a.equals(new Point(1, 2)) + " " + a.equals(new Point(2, 1)) + " " + a.equals("p"));
        Pair<String, Integer> p = new Pair<>("a", 1);
        System.out.println(p.equals(new Pair<>("a", 1)) + " " + p.equals(new Pair<>("a", 2)) + " " + p.equals(a));
        System.out.println(length("four") + " " + length("") + " " + length(4));
    }
}
