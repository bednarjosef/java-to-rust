// A constructor that throws after its object escaped: it registers `this` in a static array,
// so the object stays reachable although `new` failed, and a constructor that delegates with
// `this(...)` throws after the other one registered the object.
public class ThrowingConstructorsRegistered {
    static Item[] items = new Item[2];
    static int count = 0;

    static class Item {
        final String name;
        int uses;

        Item(String name) {
            this.name = name;
            if (count == items.length) {
                throw new IllegalStateException("full at " + name);
            }
            items[count++] = this;
        }

        Item(String name, int uses) {
            this(name);
            this.uses = uses;
            if (uses < 0) {
                throw new IllegalArgumentException("uses " + uses);
            }
        }
    }

    public static void main(String[] args) {
        String[] names = {"a", "b", "c"};
        int[] uses = {1, -2, 3};
        for (int i = 0; i < names.length; i++) {
            try {
                Item item = new Item(names[i], uses[i]);
                item.uses++;
                System.out.println(item.name + " " + item.uses);
            } catch (RuntimeException e) {
                System.out.println("no " + names[i] + ": " + e.getMessage());
            }
        }
        for (int i = 0; i < count; i++) {
            System.out.println(items[i].name + " used " + items[i].uses);
        }
    }
}
