// Lambdas passed to lent closure parameters that capture another lent closure: the closure on the
// stack borrows it, so the parameter it came from stays lent. A merger that swaps the indexes of
// the consumer it is given, a query that wraps its consumer for each section, closures nested in
// closures, a recursion that wraps its closure at each level, a generic closure type, a lambda
// lent to a parameter that may be null, a thrown exception, and the forms that keep a closure
// stored: one captured into a stored lambda, one that may be null, one in a call whose arguments
// are computed first because its receiver may be null.
public class LentClosureCaptures {
    interface IndexConsumer { boolean merge(int first, int second, int result); }

    interface Merger { boolean forMergedIndexes(IndexConsumer consumer); }

    static class Identical implements Merger {
        final int size;
        Identical(int size) { this.size = size; }
        public boolean forMergedIndexes(IndexConsumer consumer) {
            for (int i = 0; i < size; i++) {
                if (!consumer.merge(i, i, i)) return false;
            }
            return true;
        }
    }

    static class NonOverlapping implements Merger {
        final int lower, upper;
        final boolean swap;
        NonOverlapping(int lower, int upper, boolean swap) { this.lower = lower; this.upper = upper; this.swap = swap; }
        public boolean forMergedIndexes(IndexConsumer consumer) {
            return swap ? forNonSwapped((a, b, r) -> consumer.merge(b, a, r)) : forNonSwapped(consumer);
        }
        private boolean forNonSwapped(IndexConsumer consumer) {
            for (int i = 0; i < lower; i++) {
                if (!consumer.merge(i, -1, i)) return false;
            }
            for (int i = 0; i < upper; i++) {
                if (!consumer.merge(lower - 1, i, lower + i)) return false;
            }
            return true;
        }
    }

    static boolean joinIsNotEmpty(Merger x, Merger y, Merger z, int[] full) {
        return !x.forMergedIndexes(
            (x1, x2, xr) -> y.forMergedIndexes(
                (y1, y2, yr) -> z.forMergedIndexes((z1, z2, zr) -> zr != 2 || full[(xr * 4 + yr) & 15] != 0)));
    }

    interface Consumer { int accept(int entity); }
    interface SectionVisitor { int visit(int[] section); }

    static final int[][] SECTIONS = new int[16][];

    static int forEachSection(int from, int to, SectionVisitor visitor) {
        int found = 0;
        for (int i = from; i < to; i++) {
            int[] section = SECTIONS[i & 15];
            if (section.length > 0) found += visitor.visit(section);
        }
        return found;
    }

    static int getEntities(int from, int to, Consumer consumer) {
        return forEachSection(from, to, section -> {
            int n = 0;
            for (int e : section) n += consumer.accept(e);
            return n;
        });
    }

    static int getEntitiesOfType(int from, int to, int type, Consumer consumer) {
        return getEntities(from, to, e -> e % 3 == type ? consumer.accept(e) : 0);
    }

    static Consumer counting(int[] counter) {
        return e -> { counter[0]++; return 1; };
    }

    interface Step { int apply(int x); }

    static int each(int n, Step step) {
        int total = 0;
        for (int i = 0; i < n; i++) total += step.apply(i);
        return total;
    }

    static int nested(int n, Step step) {
        return each(n, i -> each(n, j -> each(2, k -> step.apply(i * 100 + j * 10 + k))));
    }

    static int wrapped(int depth, Step step) {
        if (depth == 0) return step.apply(0);
        return wrapped(depth - 1, x -> step.apply(x + depth));
    }

    static int failing(int n, Step step) {
        return each(n, i -> {
            if (i == 3) throw new IllegalStateException("step " + i);
            return step.apply(i);
        });
    }

    static Step kept;

    static int keeps(Step step) {
        kept = x -> step.apply(x) * 2;
        return each(2, step);
    }

    static int maybe(Step step) {
        return each(3, i -> step.apply(i));
    }

    interface Sink<T> { int take(T item); }

    static <T> int forAll(T[] items, Sink<T> sink) {
        int n = 0;
        for (T item : items) n += sink.take(item);
        return n;
    }

    static <T> int forAllTwice(T[] items, Sink<T> sink) {
        return forAll(items, item -> sink.take(item) * 2);
    }

    static int lengths(String[] words, Sink<String> sink) {
        return forAllTwice(words, w -> sink.take(w) + w.length());
    }

    static int eachOfThree(Step step) {
        int total = 0;
        for (int i = 0; i < 3; i++) total += step.apply(i);
        return total;
    }

    static int viaOptional(Step step) {
        return eachOfThree(x -> step.apply(x) + 1);
    }

    static final class Runner {
        int runs;
        int run(Step step, int times) {
            int total = 0;
            for (int i = 0; i < times; i++) total += step.apply(i);
            runs++;
            return total;
        }
    }

    static int timesAsked;

    static int times() {
        timesAsked++;
        return 3;
    }

    static Runner runner(int i) {
        return i > 0 ? new Runner() : null;
    }

    static int ordered(Runner r, Step step) {
        return r.run(x -> step.apply(x) + 10, times());
    }

    public static void main(String[] args) {
        for (int i = 0; i < SECTIONS.length; i++) {
            SECTIONS[i] = new int[i % 3 == 0 ? 0 : 1 + i % 4];
            for (int j = 0; j < SECTIONS[i].length; j++) SECTIONS[i][j] = i * 7 + j;
        }
        int[] full = new int[16];
        for (int i = 0; i < 16; i++) full[i] = i == 11 ? 0 : 1;
        Merger[] mergers = { new Identical(3), new NonOverlapping(2, 2, false), new NonOverlapping(1, 3, true) };
        StringBuilder joins = new StringBuilder();
        for (Merger x : mergers) {
            for (Merger y : mergers) {
                joins.append(joinIsNotEmpty(x, y, mergers[2], full) ? '1' : '0');
            }
        }
        System.out.println("joins " + joins);
        int[] merged = new int[1];
        mergers[2].forMergedIndexes((a, b, r) -> { merged[0] += a * 100 + b * 10 + r; return true; });
        System.out.println("swapped " + merged[0]);

        int threshold = 40;
        System.out.println("entities " + getEntities(0, 16, e -> e > threshold ? 1 : 0));
        System.out.println("typed " + getEntitiesOfType(2, 12, 1, e -> e));
        int[] counter = new int[1];
        System.out.println("counted " + getEntities(0, 8, counting(counter)) + " " + counter[0]);

        System.out.println("nested " + nested(3, x -> x % 7));
        System.out.println("wrapped " + wrapped(5, x -> x * 3));
        try {
            System.out.println(failing(5, x -> x));
        } catch (IllegalStateException e) {
            System.out.println("caught " + e.getMessage());
        }
        System.out.println("keeps " + keeps(x -> x + 1) + " " + kept.apply(4));
        System.out.println("maybe " + maybe(x -> x * x));
        System.out.println("generic " + lengths(new String[] { "a", "bb", "ccc" }, w -> w.charAt(0)));
        System.out.println("optional " + viaOptional(x -> x * 5));
        try {
            System.out.println(eachOfThree(null));
        } catch (NullPointerException e) {
            System.out.println("caught a null step of three");
        }
        Runner r = runner(args.length + 1);
        System.out.println("ordered " + ordered(r, x -> x * 7) + " " + r.runs + " " + timesAsked);
        try {
            System.out.println(ordered(runner(args.length), x -> x));
        } catch (NullPointerException e) {
            System.out.println("caught a null runner after " + timesAsked + " asks");
        }
        try {
            System.out.println(maybe(null));
        } catch (NullPointerException e) {
            System.out.println("caught a null step");
        }
    }
}
