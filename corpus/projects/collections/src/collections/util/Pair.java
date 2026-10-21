package collections.util;

// Two values of two type variables, compared by both.
public final class Pair<A, B> {
    private final A first;
    private final B second;

    public Pair(A first, B second) {
        this.first = first;
        this.second = second;
    }

    public A first() {
        return first;
    }

    public B second() {
        return second;
    }

    public <C> Pair<A, C> withSecond(C c) {
        return new Pair<>(first, c);
    }

    @Override
    public boolean equals(Object o) {
        return o instanceof Pair<?, ?> p && p.first.equals(first) && p.second.equals(second);
    }

    @Override
    public int hashCode() {
        return 31 * first.hashCode() + second.hashCode();
    }

    @Override
    public String toString() {
        return "(" + first + ", " + second + ")";
    }
}
