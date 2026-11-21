package visits.walk;

// Closure factories in another package than their callers, whose lambdas call this package's
// private methods and read its enum.
public interface Visitors {
    enum Step { GO_ON, STOP }

    interface Visitor { Step visit(int i); }

    interface Sink { void accept(int i); }

    static Visitor forSink(Sink sink) {
        return i -> {
            sink.accept(Shift.shifted(i));
            return Step.GO_ON;
        };
    }

    static Visitor until(int limit) { return i -> Shift.below(i, limit) ? Step.GO_ON : Step.STOP; }

    final class Shift {
        private Shift() {}

        private static int shifted(int i) { return i + 100; }

        private static boolean below(int i, int limit) { return i < limit; }
    }

    static int walk(int n, Visitor visitor) {
        int i = 0;
        while (i < n && visitor.visit(i) == Step.GO_ON) i++;
        return i;
    }
}
