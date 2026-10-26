import java.util.function.Function;

// The NullPointerExceptions of method references passed as a `Function<? super S, ? extends A>`
// and called with null: the class that the JVM makes for the reference unboxes the argument
// (`Character::toString` of an `Integer`) or calls the receiver (`Enum::toString`), and its
// hidden frame gives no message.
public class WildcardReferenceMessages {
    static class Codec<A> {
        final String name;

        Codec(String name) {
            this.name = name;
        }

        <S> Codec<S> comap(Function<? super S, ? extends A> from, S sample) {
            System.out.println(name + " " + from.apply(sample));
            return new Codec<>(name + "'");
        }
    }

    enum Color {
        RED,
        GREEN
    }

    static final Codec<String> STRING = new Codec<>("string");

    static <E extends Enum<E>> Codec<E> legacyEnum(E sample) {
        return STRING.comap(Enum::toString, sample);
    }

    static Codec<Integer> codepoint(Integer sample) {
        return STRING.comap(Character::toString, sample);
    }

    interface Action {
        void run();
    }

    static void report(Action action) {
        try {
            action.run();
        } catch (NullPointerException e) {
            System.out.println("NullPointerException: " + e.getMessage());
        }
    }

    public static void main(String[] args) {
        report(() -> codepoint(65));
        report(() -> codepoint(null));
        report(() -> legacyEnum(Color.GREEN));
        report(() -> legacyEnum((Color) null));
    }
}
