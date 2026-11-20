import java.util.ArrayList;
import java.util.List;
import java.util.function.Function;

// Closure parameters that a method only calls and passes on are lent: lambdas passed to them live
// on the caller's stack. Mixed with closures that are stored, captured by other closures, null,
// passed through interfaces, shared with a thread, generic, or thrown from.
public class LentClosures {
    interface Visitor { boolean visit(int x, int y); }
    interface Check extends Visitor {}
    interface Sink { void accept(int v); }
    interface Failing { int run(int v) throws Exception; }

    interface Walker { int walk(int n, Sink s); }

    static class Counting implements Walker {
        public int walk(int n, Sink s) {
            for (int i = 0; i < n; i++) s.accept(i);
            return n;
        }
    }

    static class Doubling implements Walker {
        public int walk(int n, Sink s) {
            for (int i = 0; i < n; i++) { s.accept(i); s.accept(i); }
            return 2 * n;
        }
    }

    interface Keeper { void keep(Sink s); }

    static class Storing implements Keeper {
        Sink kept;
        public void keep(Sink s) { kept = s; }
    }

    static class Calling implements Keeper {
        public void keep(Sink s) { s.accept(-1); }
    }

    static int sum;
    static Visitor stored = (x, y) -> x > 1;

    static boolean forEach(int n, Visitor v) {
        for (int i = 0; i < n; i++) {
            if (!v.visit(i, i * 2)) return false;
        }
        return true;
    }

    static boolean passOn(int n, Visitor v) {
        return forEach(n, v) && forEach(n / 2, v);
    }

    static int depth(int n, Visitor v) {
        if (n == 0) return 0;
        return (v.visit(n, n) ? 1 : 0) + depth(n - 1, v);
    }

    static boolean checked(int n, Check c) {
        return forEach(n, c);
    }

    static void each(int n, Sink s) {
        for (int i = 0; i < n; i++) s.accept(i);
    }

    // `s` is captured by the lambda, so it is not lent; the lambda passed to `each` is.
    static void shifted(int n, Sink s) {
        each(n, v -> s.accept(v + 1));
    }

    static boolean maybe(int n, Visitor v) {
        if (n < 0) return v.visit(n, n);
        return n > 0;
    }

    static int apply(Function<Integer, Integer> f, int times) {
        int x = 0;
        for (int i = 0; i < times; i++) x = f.apply(x);
        return x;
    }

    static int attempt(Failing f, int v) {
        try {
            return f.run(v);
        } catch (Exception e) {
            return -100 - v;
        }
    }

    public static void main(String[] args) throws Exception {
        int[] total = new int[1];
        int k = 3;
        String name = "abc";
        List<Integer> seen = new ArrayList<>();
        for (int r = 0; r < 100; r++) {
            forEach(10, (x, y) -> { total[0] += x * k + y + name.length(); return true; });
            passOn(6, (x, y) -> x < 4);
            shifted(5, v -> sum += v);
        }
        System.out.println(total[0] + " " + sum);
        System.out.println(forEach(3, stored) + " " + passOn(4, stored));
        Visitor local = (x, y) -> { sum += x; return true; };
        System.out.println(forEach(4, local) + " " + local.visit(1, 1) + " " + sum);
        Visitor either = (x, y) -> x == 0;
        if (args.length > 5) either = (x, y) -> true;
        System.out.println(forEach(2, either) + " " + depth(5, (x, y) -> x % 2 == 0));
        System.out.println(checked(5, (x, y) -> y < 6) + " " + maybe(3, null) + " " + maybe(0, null));
        try {
            maybe(-1, null);
        } catch (NullPointerException e) {
            System.out.println("null visitor");
        }
        Walker[] walkers = {new Counting(), new Doubling()};
        for (Walker w : walkers) {
            System.out.println(w.walk(3, v -> seen.add(v * k)) + " " + seen);
        }
        Storing storing = new Storing();
        Keeper[] keepers = {storing, new Calling()};
        for (Keeper keeper : keepers) keeper.keep(v -> seen.add(v));
        storing.kept.accept(7);
        System.out.println(seen);
        System.out.println(apply(x -> x * 2 + 1, 5) + " " + apply(x -> x + k, 4));
        System.out.println(attempt(v -> v * 10, 4) + " " + attempt(v -> { if (v > 2) throw new Exception("big"); return v; }, 5));
        int[] shared = new int[1];
        Sink adding = v -> { synchronized (shared) { shared[0] += v; } };
        Thread t = new Thread(() -> each(100, adding));
        t.start();
        t.join();
        each(10, adding);
        System.out.println(shared[0]);
    }
}
