// Locals declared without a value inside a `try` (or a `catch`) and assigned there, of types that
// have no default value, the way `ArrayList.batchRemove` declares `for (Object e; r < end; r++)`.
public class LocalsDeclaredInsideTry {
    static class Item {
        final String name;

        Item(String name) {
            this.name = name;
        }
    }

    static int kept(Item[] items, String skip) {
        int w = 0;
        int r = 0;
        try {
            for (Item e; r < items.length; r++) {
                if (!(e = items[r]).name.equals(skip)) {
                    items[w++] = e;
                }
            }
        } catch (RuntimeException ex) {
            System.out.println("failed at " + r);
        } finally {
            System.out.println("read " + r);
        }
        return w;
    }

    static String last(Item[] items) {
        try {
            Item found;
            found = items[0];
            for (Item item : items) {
                found = item;
            }
            return found.name;
        } catch (ArrayIndexOutOfBoundsException ex) {
            Item none;
            none = new Item("none");
            return none.name;
        }
    }

    public static void main(String[] args) {
        Item[] items = {new Item("a"), new Item("b"), new Item("a"), new Item("c")};
        int w = kept(items, "a");
        for (int i = 0; i < w; i++) {
            System.out.println(items[i].name);
        }
        Item[] broken = {new Item("x"), null, new Item("y")};
        System.out.println(kept(broken, "z"));
        System.out.println(last(items));
        System.out.println(last(new Item[0]));
    }
}
