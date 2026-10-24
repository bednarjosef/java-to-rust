// Objects held in the fields of other objects of the same method: lambdas and anonymous classes
// that capture objects, iterables whose iterators capture the iterable's captures, and objects
// made in a loop and kept by an object made outside it, which must stay alive.
import java.util.Iterator;

public class StackCaptures {
    interface Fn {
        int apply(int x);
    }

    static class Counter {
        int count;
        int add(int x) {
            count += x;
            return count;
        }
    }

    // Keeps the counter it is given: whatever holds it lets it escape.
    static class Keeper {
        static Counter kept;
        static void keep(Counter c) { kept = c; }
    }

    static int sum(int[] xs, Fn f) {
        int total = 0;
        for (int x : xs) total += f.apply(x);
        return total;
    }

    // A list like javac's: its iterator keeps the list in a field.
    static class Chain implements Iterable<Integer> {
        final int head;
        final Chain tail;
        Chain(int head, Chain tail) { this.head = head; this.tail = tail; }
        public Iterator<Integer> iterator() {
            return new Iterator<Integer>() {
                Chain at = Chain.this;
                public boolean hasNext() { return at != null; }
                public Integer next() {
                    int h = at.head;
                    at = at.tail;
                    return h;
                }
            };
        }
    }

    // A scope like javac's: an iterable lambda whose iterator captures what the lambda captured.
    static Iterable<Integer> matching(Chain chain, Fn keep) {
        return () -> new Iterator<Integer>() {
            Chain at = skip(chain, keep);
            public boolean hasNext() { return at != null; }
            public Integer next() {
                int h = at.head;
                at = skip(at.tail, keep);
                return h;
            }
        };
    }

    static Chain skip(Chain c, Fn keep) {
        while (c != null && keep.apply(c.head) == 0) c = c.tail;
        return c;
    }

    // A mapping that hands itself to a lambda it makes, like javac's TypeMapping.
    static class Scale {
        final int by;
        Scale(int by) { this.by = by; }
        int one(int x) { return x * by; }
        int all(int[] xs) { return sum(xs, x -> one(x)); }
    }

    static class Box {
        Counter c;
        Box(Counter c) { this.c = c; }
        int bump() { return c.add(1); }
    }

    // Its field is read only by its own methods.
    static class Cell {
        final Counter v;
        Cell(Counter v) { this.v = v; }
        int get() { return v.count; }
    }

    public static void main(String[] args) {
        int[] xs = { 1, 2, 3, 4 };
        long total = 0;
        for (int round = 0; round < 1000; round++) {
            Counter c = new Counter();
            total += sum(xs, x -> c.add(x));
            Counter k = new Counter();
            total += sum(xs, x -> {
                Keeper.keep(k);
                return k.add(x);
            });
            total += new Scale(round % 5).all(xs);
            // One capture of a lambda escapes and the other does not.
            Counter mine = new Counter();
            Counter given = new Counter();
            total += sum(xs, x -> {
                Keeper.keep(given);
                return mine.add(x) + given.add(1);
            });
        }
        System.out.println(total + " " + Keeper.kept.count);

        Chain chain = null;
        for (int i = 10; i > 0; i--) chain = new Chain(i, chain);
        long seen = 0;
        for (int round = 0; round < 1000; round++) {
            for (int v : chain) seen += v;
            final int mod = round % 3 + 2;
            for (int v : matching(chain, x -> x % mod == 0 ? 1 : 0)) seen += v;
        }
        System.out.println(seen);

        // Each box made in the loop holds that iteration's counter; the boxes outlive it.
        Box[] boxes = new Box[5];
        Box last = null;
        int bumps = 0;
        for (int i = 0; i < 5; i++) {
            Counter boxed = new Counter();
            boxed.add(i);
            Box b = new Box(boxed);
            if (last != null) bumps += last.bump();
            last = b;
            boxes[i] = b;
        }
        for (Box b : boxes) bumps += b.bump();
        System.out.println(bumps + " " + last.c.count);

        // A counter made in a loop and held by a box made before it: the box holds each in turn.
        Box holder = new Box(new Counter());
        int held = 0;
        for (int i = 0; i < 5; i++) {
            Counter next = new Counter();
            next.add(10 * i);
            holder.c = next;
            if (i > 0) held += holder.bump();
        }
        System.out.println(held + " " + holder.c.count);

        // Each cell made in the loop holds that iteration's counter, and the next iteration
        // still reads the previous cell, so neither may be made again in the same place.
        Cell prev = null;
        int got = 0;
        for (int i = 0; i < 5; i++) {
            Counter inCell = new Counter();
            inCell.add(100 + i);
            Cell cell = new Cell(inCell);
            if (prev != null) got += prev.get();
            prev = cell;
        }
        // A cell and its counter that live only in one iteration.
        for (int i = 0; i < 5; i++) {
            Counter once = new Counter();
            once.add(i);
            got += new Cell(once).get();
        }
        System.out.println(got + " " + prev.get());
    }
}
