public class Identity {
    static abstract class Item {
        String id;
        Item next;
        Item(String id) { this.id = id; }
        abstract int price();
        Item self() { return this; }
        boolean isSelf(Item other) { return other == this; }
        public String toString() { return id + "@" + price(); }
    }

    static class Book extends Item {
        int pages;
        Book(String id, int pages) { super(id); this.pages = pages; }
        int price() { return pages / 10; }
        Book chain(Item n) { next = n; return this; }
    }

    static class Pen extends Item {
        Pen(String id) { super(id); }
        int price() { return 2; }
    }

    static Item cheapest(Item[] items) {
        Item best = null;
        for (Item it : items) {
            if (best == null || it.price() < best.price()) best = it;
        }
        return best;
    }

    static int length(Item first) {
        int n = 0;
        for (Item at = first; at != null; at = at.next) n++;
        return n;
    }

    public static void main(String[] args) {
        Book b = new Book("b1", 120);
        Pen p = new Pen("p1");
        Item i1 = b;
        Item i2 = p;
        Item i3 = b;
        System.out.println((i1 == i3) + " " + (i1 == i2) + " " + (i1 != i2) + " " + (i1 == b) + " " + (b == i1));
        System.out.println(i1.self() == b);
        System.out.println(i2.isSelf(p) + " " + i2.isSelf(b));
        Item none = null;
        System.out.println((none == null) + " " + (none == i1) + " " + (i1 == none));
        Item[] items = new Item[3];
        items[0] = b;
        items[1] = p;
        items[2] = new Book("b2", 5);
        System.out.println(cheapest(items));
        System.out.println(items[2] == cheapest(items));
        b.chain(p).chain(items[2]);
        System.out.println(length(b) + " " + length(items[2]) + " " + length(null));
        System.out.println(b.next == p);
        System.out.println(b.next);
        Item last = items[2];
        last.next = null;
        System.out.println("last " + last + " " + last.next);
    }
}
