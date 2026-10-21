// Records with an explicit canonical constructor, an overridden accessor, `Comparable`, components
// that are objects, boxes and nulls, `java.util.Objects` and a record declared in a method.
import java.util.Objects;

public class RecordObjectMethods {
    record Money(long cents) implements Comparable<Money> {
        Money(long cents) {
            if (cents < 0) {
                this.cents = 0;
            } else {
                this.cents = cents;
            }
        }

        public long cents() {
            return cents * 1;
        }

        public int compareTo(Money other) {
            return Long.compare(cents, other.cents);
        }
    }

    record Named(String name, int count) {}

    static final class Plain {
        final int v;
        Plain(int v) { this.v = v; }
        @Override public boolean equals(Object o) {
            return o instanceof Plain p && p.v == v;
        }
        @Override public int hashCode() { return Objects.hash(v); }
    }

    record Holder(Plain plain, Integer boxed, Double d) {}

    public static void main(String[] args) {
        Money a = new Money(-5);
        Money b = new Money(250);
        System.out.println(a + " " + b + " " + a.compareTo(b) + " " + b.cents());
        Named n = new Named("x", 2);
        System.out.println(n.name() + " " + n.count());
        Holder h = new Holder(new Plain(3), 7, null);
        Holder g = new Holder(new Plain(3), 7, null);
        System.out.println(h.equals(g) + " " + h.hashCode() + " " + (h.hashCode() == g.hashCode()));
        Holder k = new Holder(null, null, 2.0);
        System.out.println(k + " " + k.equals(h) + " " + k.hashCode());
        System.out.println(Objects.equals("a", "a") + " " + Objects.hashCode(null) + " " + Objects.hash(1, "b", 2.5));
        record Local(int q) {}
        System.out.println(new Local(4));
    }
}
