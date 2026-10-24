import java.util.Iterator;
import java.util.NoSuchElementException;

public class StackObjectsInLists {
    interface Mapper<A, B> { B apply(A a); }

    static class List<A> implements Iterable<A> {
        A head;
        List<A> tail;
        static final List<Object> EMPTY = new List<>(null, null);
        List(A head, List<A> tail) { this.head = head; this.tail = tail; }
        @SuppressWarnings("unchecked")
        static <A> List<A> nil() { return (List<A>) EMPTY; }
        static <A> List<A> of(A x) { return new List<>(x, nil()); }
        boolean isEmpty() { return tail == null; }
        public Iterator<A> iterator() {
            if (tail == null) return emptyIterator();
            return new Iterator<A>() {
                List<A> elems = List.this;
                public boolean hasNext() { return elems.tail != null; }
                public A next() {
                    if (elems.tail == null) throw new NoSuchElementException();
                    A result = elems.head;
                    elems = elems.tail;
                    return result;
                }
            };
        }
        <Z> List<Z> map(Mapper<A, Z> mapper) {
            boolean changed = false;
            ListBuffer<Z> buf = new ListBuffer<>();
            for (A a : this) {
                Z z = mapper.apply(a);
                buf.append(z);
                changed |= (z != a);
            }
            @SuppressWarnings("unchecked")
            List<Z> same = (List<Z>) this;
            return changed ? buf.toList() : same;
        }
    }

    @SuppressWarnings("rawtypes")
    static final Iterator EMPTY_ITERATOR = new Iterator() {
        public boolean hasNext() { return false; }
        public Object next() { throw new NoSuchElementException(); }
    };

    @SuppressWarnings("unchecked")
    static <A> Iterator<A> emptyIterator() { return (Iterator<A>) EMPTY_ITERATOR; }

    static class ListBuffer<A> {
        List<A> elems = List.nil();
        List<A> last;
        int count;
        boolean shared;
        ListBuffer<A> append(A x) {
            if (shared) copy();
            List<A> newLast = List.of(x);
            if (last != null) { last.tail = newLast; last = newLast; } else { elems = last = newLast; }
            count++;
            return this;
        }
        void copy() {
            List<A> orig = elems;
            elems = last = List.nil();
            for (A a : new ListIterable<>(orig)) append(a);
        }
        List<A> toList() { shared = true; return elems; }
    }

    static class ListIterable<A> implements Iterable<A> {
        final List<A> list;
        ListIterable(List<A> list) { this.list = list; }
        public Iterator<A> iterator() { return list.iterator(); }
    }

    static class Num {
        final int v;
        Num(int v) { this.v = v; }
    }

    static class Visitor {
        int depth;
        Num visit(Num t, Num s) { return new Num(t.v + s.v + depth); }
        List<Num> visit(List<Num> ts, Num s) { return ts.map(t -> visit(t, s)); }
    }

    public static void main(String[] args) {
        ListBuffer<Num> lb = new ListBuffer<>();
        for (int i = 0; i < 10; i++) lb.append(new Num(i));
        List<Num> xs = lb.toList();
        Visitor v = new Visitor();
        long total = 0;
        for (int round = 0; round < 2000; round++) {
            List<Num> ys = v.visit(xs, new Num(round));
            for (Num y : ys) total += y.v;
            List<Num> zs = xs.map(new Mapper<Num, Num>() {
                public Num apply(Num a) { return a; }
            });
            if (zs != xs) total = -1;
        }
        System.out.println(total);
    }
}
