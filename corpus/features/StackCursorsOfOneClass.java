// A cursor whose class's `next()` never returns the cursor stays on the stack, though another
// class's `next()` returns its receiver: a call on an object of the method's own returns what
// that object's class runs returns, not what any class's might.
public class StackCursorsOfOneClass {
    interface Cursor<T> {
        boolean hasNext();
        T next();
    }

    static final class Item {
        final int v;
        Item(int v) { this.v = v; }
    }

    static final class Bag {
        final Item[] items = new Item[16];
        int n;
        void add(Item item) { items[n++] = item; }
        Cursor<Item> cursor() { return new BagCursor(this); }
    }

    // Walks a bag's items: `next()` returns one of them.
    static final class BagCursor implements Cursor<Item> {
        final Bag bag;
        int i;
        BagCursor(Bag bag) { this.bag = bag; }
        public boolean hasNext() { return i < bag.n; }
        public Item next() { return bag.items[i++]; }
    }

    // Its own element, as some map iterators are: `next()` returns the cursor itself.
    static final class Countdown implements Cursor<Countdown> {
        int left;
        Countdown(int left) { this.left = left; }
        public boolean hasNext() { return left > 0; }
        public Countdown next() { left--; return this; }
    }

    static final Object[] kept = new Object[64];
    static int count;

    static void keep(Object o) { kept[count++] = o; }

    // Gives back the cursor it is given, so that a call on its result may run either class's
    // `next()`, whichever cursor a caller gave it.
    static <T> Cursor<T> same(Cursor<T> c) { return c; }

    public static void main(String[] args) {
        Bag bag = new Bag();
        for (int i = 0; i < 10; i++) {
            bag.add(new Item(i * 3));
        }
        int sum = 0;
        for (int round = 0; round < 3; round++) {
            Cursor<Item> c = same(bag.cursor());
            while (c.hasNext()) {
                Item item = c.next();
                keep(item);
                sum += item.v;
            }
        }
        // The countdown is its own element, so keeping the element keeps it.
        Cursor<Countdown> d = same(new Countdown(4));
        int left = 0;
        while (d.hasNext()) {
            Countdown x = d.next();
            keep(x);
            left += x.left;
        }
        System.out.println(sum + " " + left + " " + count + " " + (kept[count - 1] == d));
    }
}
