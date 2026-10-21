// Constructors whose first assignments read fields that earlier ones set, as `EnumMap`'s does
// (`keyUniverse = ..; vals = new Object[keyUniverse.length]`): every field is set before
// anything can read it, so the array and the object fields are never null.
public class ConstructorReadsEarlierFields {
    static int made;

    static String[] universe(int n) {
        made++;
        String[] keys = new String[n];
        for (int i = 0; i < n; i++) {
            keys[i] = "k" + i;
        }
        return keys;
    }

    static final class Table {
        final int size;
        final String[] keys;
        final Object[] values;
        final int capacity;
        final String label;
        int puts = made * 100;

        Table(int size) {
            this.size = size;
            keys = universe(size);
            values = new Object[keys.length];
            capacity = values.length * 2 + this.size;
            label = keys[0] + ".." + keys[keys.length - 1];
        }

        void put(int i, Object value) {
            values[i] = value;
            puts++;
        }

        String show() {
            StringBuilder sb = new StringBuilder(label).append(" cap ").append(capacity).append(" puts ").append(puts);
            for (int i = 0; i < size; i++) {
                sb.append(' ').append(keys[i]).append('=').append(values[i]);
            }
            return sb.toString();
        }
    }

    public static void main(String[] args) {
        Table t = new Table(3);
        t.put(1, "x");
        t.put(2, 42);
        System.out.println(t.show());
        Table u = new Table(1);
        System.out.println(u.show() + " made " + made);
    }
}
