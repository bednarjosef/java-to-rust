// A downcast that a `catch` observes, of a value whose every created class is under the target:
// the cast cannot fail, as the JDK's `TreeMap.KeySet` casts its `NavigableMap` field to `TreeMap`
// although another class (here `Unused`) implements the interface too.
public class CastsUnderEveryCreatedClass {
    interface Store {
        int size();
    }

    static class Ordered implements Store {
        final int[] keys;

        Ordered(int... keys) {
            this.keys = keys;
        }

        public int size() {
            return keys.length;
        }

        int first() {
            return keys.length == 0 ? -1 : keys[0];
        }
    }

    static final class Reversed extends Ordered {
        Reversed(int... keys) {
            super(keys);
        }

        @Override
        int first() {
            return keys.length == 0 ? -1 : keys[keys.length - 1];
        }
    }

    static final class Unused implements Store {
        public int size() {
            return 0;
        }
    }

    static class View {
        final Store store;

        View(Store store) {
            this.store = store;
        }

        int first() {
            return ((Ordered) store).first();
        }
    }

    public static void main(String[] args) {
        View[] views = {new View(new Ordered(3, 1, 2)), new View(new Reversed(3, 1, 2)), new View(new Ordered())};
        for (View v : views) {
            try {
                System.out.println(v.store.size() + " " + v.first());
            } catch (ClassCastException e) {
                System.out.println("not ordered");
            }
        }
    }
}
