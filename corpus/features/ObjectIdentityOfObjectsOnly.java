// `==` on `Object` values where no string or box can reach either side: an identity map in the
// manner of Are We Fast Yet's IdentityDictionary, whose keys are only ever the program's objects,
// while its values and other `Object`s of the program are strings and boxes. Its `==` is plain
// identity, so two equal-looking keys are different keys.
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
        Object text = "text";
        Object number = 42;
        System.out.println(text.equals("text") + " " + number.equals(42) + " " + map.get(bb, 2).equals("two"));
    }
}
