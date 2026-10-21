// Unboxing a null throws a `NullPointerException` that a `catch` sees: by `intValue()`, as the
// JDK's `Sink.OfInt.accept(Integer)` does, and by arithmetic on a box.
public class UnboxingNullCaught {
    interface OfInt {
        void accept(int value);

        default void accept(Integer i) {
            accept(i.intValue());
        }
    }

    public static void main(String[] args) {
        OfInt sink = v -> System.out.println("got " + v);
        Integer missing = args.length > 5 ? 3 : null;
        sink.accept(Integer.valueOf(4));
        try {
            sink.accept(missing);
            System.out.println("not here");
        } catch (NullPointerException e) {
            System.out.println("caught");
        }
        Integer seven = 7;
        try {
            int n = missing + seven;
            System.out.println(n);
        } catch (NullPointerException e) {
            System.out.println("caught again");
        }
    }
}
