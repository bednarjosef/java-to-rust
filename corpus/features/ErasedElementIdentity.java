// Elements of an array that the translation holds as an `Object[]` (it is passed where one is
// taken), and a generic member's value, compared by identity: javac casts neither side of `==`.
public class ErasedElementIdentity {
    interface Key {
        String name();
    }

    static final class Named implements Key {
        final String n;

        Named(String n) {
            this.n = n;
        }

        public String name() {
            return n;
        }
    }

    static final class Other implements Key {
        public String name() {
            return "other";
        }
    }

    static final class Holder<T> {
        T value;

        Holder(T value) {
            this.value = value;
        }
    }

    static int count(Object[] all) {
        int n = 0;
        for (Object o : all) {
            if (o != null) {
                n++;
            }
        }
        return n;
    }

    static int index(Key[] keys, Key key) {
        for (int i = 0; i < keys.length; i++) {
            if (keys[i] == key) {
                return i;
            }
        }
        return -1;
    }

    static boolean differs(Key[] keys, int i, Key key) {
        return keys[i] != key;
    }

    public static void main(String[] args) {
        Key a = new Named("a");
        Key b = new Other();
        Key c = new Named("c");
        Key[] keys = {a, b, null, c};
        System.out.println(count(keys));
        System.out.println(index(keys, a) + " " + index(keys, b) + " " + index(keys, c) + " "
            + index(keys, new Named("a")) + " " + index(keys, null));
        System.out.println(differs(keys, 0, a) + " " + differs(keys, 2, null) + " " + differs(keys, 1, c));
        System.out.println(keys[0] == keys[3] ? "same" : keys[0].name() + " " + keys[3].name());
        Holder<Key> held = new Holder<>(a);
        System.out.println((held.value == a) + " " + (held.value == b) + " " + (held.value != c));
    }
}
