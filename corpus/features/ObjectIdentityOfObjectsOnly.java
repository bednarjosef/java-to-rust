// `==` on `Object` values where no string or box can reach one side: an identity map in the
// manner of Are We Fast Yet's IdentityDictionary, whose keys are only ever the program's objects,
// while its values and other `Object`s of the program are strings and boxes. Its `==` is plain
// identity, so two equal-looking keys are different keys. The same for an identity set that
// inherits `add` from a set that also holds boxes, as Are We Fast Yet's IdentitySet does.
public class ObjectIdentityOfObjectsOnly {
    static class Entry {
        final int hash;
        final Object key;
        Object value;
        Entry next;

        Entry(int hash, Object key, Object value, Entry next) {
            this.hash = hash;
            this.key = key;
            this.value = value;
            this.next = next;
        }

        boolean match(int hash, Object key) {
            return this.hash == hash && this.key == key;
        }
    }

    static class IdentityMap {
        private final Entry[] buckets = new Entry[8];

        void put(Object key, int hash, Object value) {
            int i = hash & 7;
            for (Entry e = buckets[i]; e != null; e = e.next) {
                if (e.match(hash, key)) {
                    e.value = value;
                    return;
                }
            }
            buckets[i] = new Entry(hash, key, value, buckets[i]);
        }

        Object get(Object key, int hash) {
            for (Entry e = buckets[hash & 7]; e != null; e = e.next) {
                if (e.match(hash, key)) {
                    return e.value;
                }
            }
            return null;
        }
    }

    static class Set {
        private final Object[] items = new Object[8];
        int size;

        boolean contains(Object o) {
            for (int i = 0; i < size; i++) {
                if (items[i].equals(o)) {
                    return true;
                }
            }
            return false;
        }

        void add(Object o) {
            if (!contains(o)) {
                items[size++] = o;
            }
        }

        boolean has(int i, Object o) {
            return i < size && test(items[i], o);
        }

        boolean test(Object item, Object o) {
            return item.equals(o);
        }
    }

    static class IdentitySet extends Set {
        @Override
        boolean contains(Object o) {
            for (int i = 0; i < 8; i++) {
                if (has(i, o)) {
                    return true;
                }
            }
            return false;
        }

        @Override
        boolean test(Object item, Object o) {
            return o == item;
        }
    }

    static class Block {
        final String name;

        Block(String name) {
            this.name = name;
        }

        int customHash() {
            return name.length();
        }
    }

    public static void main(String[] args) {
        IdentityMap map = new IdentityMap();
        Block a = new Block("a");
        Block alsoA = new Block("a");
        Block bb = new Block("bb");
        map.put(a, a.customHash(), 1);
        map.put(bb, bb.customHash(), "two");
        map.put(a, a.customHash(), 3);
        System.out.println(map.get(a, a.customHash()) + " " + map.get(alsoA, alsoA.customHash()) + " "
                + map.get(bb, bb.customHash()));
        Set numbers = new Set();
        numbers.add(300);
        numbers.add(300);
        IdentitySet blocks = new IdentitySet();
        blocks.add(a);
        blocks.add(alsoA);
        blocks.add(a);
        System.out.println(numbers.size + " " + blocks.size + " " + blocks.contains(a) + " " + blocks.contains(bb));
        Object text = "text";
        Object number = 42;
        System.out.println(text.equals("text") + " " + number.equals(42) + " " + map.get(bb, 2).equals("two"));
    }
}
