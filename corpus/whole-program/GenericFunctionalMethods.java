import java.util.ArrayList;
import java.util.List;

// Method references whose functional interface's method is generic, as the Minecraft server's
// `Palette.Factory` has `<A> Palette<A> create(int bits, List<A> entries)` made by
// `SingleValuePalette::create` and the like.
public class GenericFunctionalMethods {
    interface Palette<A> {
        A first();
        int size();
    }

    interface Factory {
        <A> Palette<A> create(int bits, List<A> entries);
    }

    static final class Linear<A> implements Palette<A> {
        private final List<A> entries;

        Linear(int bits, List<A> entries) {
            this.entries = new ArrayList<>(entries);
        }

        static <A> Palette<A> create(int bits, List<A> entries) {
            return new Linear<>(bits, entries);
        }

        public A first() {
            return entries.get(0);
        }

        public int size() {
            return entries.size();
        }
    }

    static final class Single<A> implements Palette<A> {
        private final A value;

        Single(A value) {
            this.value = value;
        }

        static <A> Palette<A> create(int bits, List<A> entries) {
            return new Single<>(entries.get(0));
        }

        public A first() {
            return value;
        }

        public int size() {
            return 1;
        }
    }

    private static final Factory LINEAR = Linear::create;
    private static final Factory SINGLE = Single::create;

    public static void main(String[] args) {
        Palette<String> words = LINEAR.create(4, List.of("stone", "dirt"));
        Palette<Integer> numbers = SINGLE.create(0, List.of(7, 8));
        System.out.println(words.first() + " " + words.size());
        System.out.println(numbers.first() + " " + numbers.size());
        for (Factory f : new Factory[] {LINEAR, SINGLE}) {
            System.out.println(f.create(2, List.of('a', 'b', 'c')).size());
        }
    }
}
