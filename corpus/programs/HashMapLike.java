// A chained hash map of two type variables, erased: keys hash and compare through `Object`'s
// `hashCode` and `equals`, which a key class may override.
public class HashMapLike {
    static class Entry<K, V> {
        final K key;
        V value;
        Entry<K, V> next;

        Entry(K key, V value, Entry<K, V> next) {
            this.key = key;
            this.value = value;
            this.next = next;
        }
    }

    static class MyMap<K, V> {
        private Entry<K, V>[] table;
        private int size;

        @SuppressWarnings("unchecked")
        MyMap() {
            table = (Entry<K, V>[]) new Entry[4];
        }

        private int indexFor(Object key, int length) {
            int h = key.hashCode();
            return (h ^ (h >>> 16)) & (length - 1);
        }

        V get(K key) {
            for (Entry<K, V> e = table[indexFor(key, table.length)]; e != null; e = e.next) {
                if (e.key.equals(key)) return e.value;
            }
            return null;
        }

        V put(K key, V value) {
            int i = indexFor(key, table.length);
            for (Entry<K, V> e = table[i]; e != null; e = e.next) {
                if (e.key.equals(key)) {
                    V old = e.value;
                    e.value = value;
                    return old;
                }
            }
            table[i] = new Entry<>(key, value, table[i]);
            if (++size > table.length * 3 / 4) resize();
            return null;
        }

        boolean containsKey(K key) {
            return get(key) != null;
        }

        @SuppressWarnings("unchecked")
        private void resize() {
            Entry<K, V>[] old = table;
            table = (Entry<K, V>[]) new Entry[old.length * 2];
            for (Entry<K, V> head : old) {
                for (Entry<K, V> e = head; e != null; ) {
                    Entry<K, V> next = e.next;
                    int i = indexFor(e.key, table.length);
                    e.next = table[i];
                    table[i] = e;
                    e = next;
                }
            }
        }

        int size() { return size; }
    }

    static class Word {
        final String text;
        Word(String text) { this.text = text; }
        @Override public boolean equals(Object o) { return o instanceof Word w && w.text.equals(text); }
        @Override public int hashCode() { return text.length(); }
        @Override public String toString() { return text; }
    }

    public static void main(String[] args) {
        MyMap<String, Integer> counts = new MyMap<>();
        String[] words = "the quick brown fox jumps over the lazy dog the end".split(" ");
        for (String w : words) {
            Integer c = counts.get(w);
            counts.put(w, c == null ? 1 : c + 1);
        }
        System.out.println(counts.size() + " " + counts.get("the") + " " + counts.get("fox") + " " + counts.get("cat"));
        MyMap<Word, String> byWord = new MyMap<>();
        byWord.put(new Word("one"), "1");
        byWord.put(new Word("two"), "2");
        byWord.put(new Word("three"), "3");
        System.out.println(byWord.get(new Word("two")) + " " + byWord.containsKey(new Word("six")) + " " + byWord.size());
        MyMap<Integer, Word> byNumber = new MyMap<>();
        for (int i = 0; i < 20; i++) byNumber.put(i * 7, new Word("w" + i));
        System.out.println(byNumber.get(49) + " " + byNumber.get(50) + " " + byNumber.size());
    }
}
