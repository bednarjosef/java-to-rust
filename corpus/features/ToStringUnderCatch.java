// A `toString` that may throw, run to make an object's text in a method whose caller catches the
// same exception: the caller's `catch` sees the method's own `throw`, and the `toString`, which
// does not throw here, keeps its plain signature.
public class ToStringUnderCatch {
    static class Item {
        final int n;

        Item(int n) {
            this.n = n;
        }

        @Override
        public String toString() {
            if (n < 0) {
                throw new IllegalStateException("negative item");
            }
            return "Item " + n;
        }
    }

    static String show(Item item) {
        if (item.n > 100) {
            throw new IllegalStateException("too big: " + item.n);
        }
        String text = "show " + item;
        System.out.println(text);
        return text;
    }

    static void both() {
        System.out.println(String.valueOf(new Item(2)) + " " + new StringBuilder().append(new Item(3)));
    }

    public static void main(String[] args) {
        try {
            show(new Item(1));
            both();
            show(new Item(200));
            System.out.println("not reached");
        } catch (IllegalStateException e) {
            System.out.println("caught " + e.getMessage());
        }
    }
}
