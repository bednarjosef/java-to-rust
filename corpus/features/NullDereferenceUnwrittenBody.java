// A catch of NullPointerException around a call whose base method is overridden by every class
// with objects: the base's body is never written, so its dereferences need no checked form, as
// with AbstractCollection.containsAll's for-each under AbstractSet.equals when every set
// overrides containsAll.
import java.util.Iterator;

public class NullDereferenceUnwrittenBody {
    static class Range implements Iterable<Integer> {
        final int end;

        Range(int end) {
            this.end = end;
        }

        int size() {
            return end;
        }

        public Iterator<Integer> iterator() {
            return new Iterator<Integer>() {
                int next = 0;

                public boolean hasNext() {
                    return next < end;
                }

                public Integer next() {
                    return next++;
                }
            };
        }
    }

    abstract static class Summer {
        int total(Range xs) {
            int t = 0;
            for (int x : xs) {
                t += x;
            }
            return t;
        }
    }

    static class Counter extends Summer {
        @Override
        int total(Range xs) {
            return xs.size();
        }
    }

    static int safely(Summer s, Range xs) {
        try {
            return s.total(xs);
        } catch (NullPointerException e) {
            return -1;
        }
    }

    static Range make(int end) {
        return end < 0 ? null : new Range(end);
    }

    public static void main(String[] args) {
        Summer s = new Counter();
        System.out.println(safely(s, make(4)));
        System.out.println(safely(s, make(-1)));
    }
}
