import java.util.function.Function;

// The intersection types that javac infers: the result of a generic call whose arguments are
// subclasses of one class that implement one interface (`Mark & Anchor`), a lambda or a method
// reference that returns one, and a `?:` or a `switch` whose branches are of different classes.
// A value of such a type is of its erasure, and calls the methods of its other bounds through a
// cast.
public class InferredIntersections {
    interface Anchor {
        int resolve(int bottom);
    }

    abstract static class Mark {
        abstract String label();

        public String toString() {
            return label();
        }
    }

    static final class Absolute extends Mark implements Anchor {
        final int y;

        Absolute(int y) {
            this.y = y;
        }

        public int resolve(int bottom) {
            return y;
        }

        String label() {
            return "absolute " + y;
        }
    }

    static final class AboveBottom extends Mark implements Anchor {
        final int offset;

        AboveBottom(int offset) {
            this.offset = offset;
        }

        public int resolve(int bottom) {
            return bottom + offset;
        }

        String label() {
            return "above bottom " + offset;
        }
    }

    static final class Either<L, R> {
        final L left;
        final R right;

        Either(L left, R right) {
            this.left = left;
            this.right = right;
        }

        <U> U map(Function<? super L, ? extends U> l, Function<? super R, ? extends U> r) {
            return left != null ? l.apply(left) : r.apply(right);
        }
    }

    static Anchor merge(Either<Absolute, AboveBottom> either) {
        return (Anchor) either.map(Function.identity(), a -> (Mark) a);
    }

    static Anchor fromNumber(Either<Integer, AboveBottom> either) {
        return either.map(Absolute::new, a -> a);
    }

    static Anchor pick(int which) {
        var anchor = switch (which) {
            case 0 -> new Absolute(3);
            default -> new AboveBottom(4);
        };
        return anchor;
    }

    public static void main(String[] args) {
        System.out.println(merge(new Either<>(new Absolute(5), null)).resolve(10));
        System.out.println(merge(new Either<>(null, new AboveBottom(2))).resolve(10));
        System.out.println(fromNumber(new Either<>(9, null)).resolve(0));
        System.out.println(fromNumber(new Either<>(null, new AboveBottom(1))).resolve(1));
        var lub = args.length > 5 ? new Absolute(1) : new AboveBottom(6);
        System.out.println(lub.resolve(1) + " " + lub);
        System.out.println(pick(0).resolve(0) + " " + pick(1).resolve(1));
        var mapped = new Either<Absolute, AboveBottom>(null, new AboveBottom(7)).map(a -> a, b -> b);
        System.out.println(mapped.resolve(2) + " " + mapped.label());
    }
}
