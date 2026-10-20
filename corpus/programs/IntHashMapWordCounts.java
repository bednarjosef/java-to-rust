public class IntHashMapWordCounts {
    /** Open addressing with linear probing; the capacity is always a power of two. */
    static class Table {
        private int[] keys;
        private int[] values;
        private boolean[] used;
        private int size;

        Table(int capacity) {
            keys = new int[capacity];
            values = new int[capacity];
            used = new boolean[capacity];
        }

        int size() {
            return size;
        }

        int capacity() {
            return keys.length;
        }

        static int mix(int key) {
            int h = key * 0x9E3779B1;
            h ^= h >>> 15;
            h *= 0x85EBCA6B;
            h ^= h >>> 13;
            return h;
        }

        /** Index of the slot holding key, or the free slot where it would be stored. */
        private int slotFor(int key) {
            int mask = keys.length - 1;
            int i = mix(key) & mask;
            while (used[i] && keys[i] != key) {
                i = (i + 1) & mask;
            }
            return i;
        }

        boolean contains(int key) {
            return used[slotFor(key)];
        }

        int get(int key, int fallback) {
            int i = slotFor(key);
            return used[i] ? values[i] : fallback;
        }

        void put(int key, int value) {
            if ((size + 1) * 4 > keys.length * 3) {
                grow();
            }
            int i = slotFor(key);
            if (!used[i]) {
                used[i] = true;
                keys[i] = key;
                size++;
            }
            values[i] = value;
        }

        private void grow() {
            int[] oldKeys = keys;
            int[] oldValues = values;
            boolean[] oldUsed = used;
            keys = new int[oldKeys.length * 2];
            values = new int[oldKeys.length * 2];
            used = new boolean[oldKeys.length * 2];
            size = 0;
            for (int i = 0; i < oldKeys.length; i++) {
                if (oldUsed[i]) {
                    put(oldKeys[i], oldValues[i]);
                }
            }
        }

        /** Removes key. The rest of its probe run is re-inserted so that lookups still succeed. */
        boolean remove(int key) {
            int mask = keys.length - 1;
            int i = slotFor(key);
            if (!used[i]) {
                return false;
            }
            used[i] = false;
            size--;
            int j = (i + 1) & mask;
            while (used[j]) {
                int movedKey = keys[j];
                int movedValue = values[j];
                used[j] = false;
                size--;
                put(movedKey, movedValue);
                j = (j + 1) & mask;
            }
            return true;
        }
    }

    /** A word's hash as a plain int: each character folds into a 31-based polynomial. */
    static int hashWord(String word) {
        int h = 17;
        for (int i = 0; i < word.length(); i++) {
            h = h * 31 + word.charAt(i);
        }
        return h;
    }

    static final String TEXT =
        "the cat sat on the mat and the dog sat on the log while a bird watched the cat "
        + "and the cat watched the bird and the dog dreamed of the mat and the log";

    /** Splits text on spaces, returning the words in order. */
    static String[] words(String text) {
        int count = 0;
        for (int i = 0; i < text.length(); i++) {
            if (text.charAt(i) == ' ' && (i == 0 || text.charAt(i - 1) != ' ')) {
                count++;
            }
        }
        if (text.length() > 0 && text.charAt(text.length() - 1) != ' ') {
            count++;
        }
        String[] out = new String[count];
        int next = 0;
        String current = "";
        for (int i = 0; i <= text.length(); i++) {
            if (i == text.length() || text.charAt(i) == ' ') {
                if (current.length() > 0) {
                    out[next] = current;
                    next++;
                }
                current = "";
            } else {
                current = current + text.charAt(i);
            }
        }
        return out;
    }

    static void countWords(String[] words, Table counts, String[] names) {
        int distinct = 0;
        for (String w : words) {
            int h = hashWord(w);
            if (!counts.contains(h)) {
                names[distinct] = w;
                distinct++;
            }
            counts.put(h, counts.get(h, 0) + 1);
        }
        System.out.println("distinct words: " + distinct);
    }

    static void showTopWords(Table counts, String[] words, int limit) {
        String[] seen = new String[words.length];
        int[] freq = new int[words.length];
        int distinct = 0;
        for (String w : words) {
            int h = hashWord(w);
            boolean found = false;
            for (int k = 0; k < distinct; k++) {
                if (w.equals(seen[k])) {
                    found = true;
                }
            }
            if (!found) {
                seen[distinct] = w;
                freq[distinct] = counts.get(h, 0);
                distinct++;
            }
        }
        for (int round = 0; round < limit && round < distinct; round++) {
            int best = round;
            for (int k = round + 1; k < distinct; k++) {
                if (freq[k] > freq[best]) {
                    best = k;
                }
            }
            String tmpWord = seen[round];
            seen[round] = seen[best];
            seen[best] = tmpWord;
            int tmpFreq = freq[round];
            freq[round] = freq[best];
            freq[best] = tmpFreq;
            System.out.println("  " + seen[round] + " " + freq[round]);
        }
    }

    public static void main(String[] args) {
        Table squares = new Table(4);
        for (int i = 1; i <= 1000; i++) {
            squares.put(i, i * i);
        }
        System.out.println("squares: size " + squares.size() + ", capacity " + squares.capacity());
        System.out.println("get(30) = " + squares.get(30, -1) + ", get(5000) = " + squares.get(5000, -1));

        long sum = 0;
        for (int i = 1; i <= 1000; i += 2) {
            if (squares.remove(i)) {
                sum += squares.get(i + 1, 0);
            }
        }
        System.out.println("removed odd keys, sum of their even successors' squares: " + sum);
        System.out.println("size after removal: " + squares.size() + ", contains 3: " + squares.contains(3));
        System.out.println("contains 4: " + squares.contains(4) + ", value of 4: " + squares.get(4, -1));

        Table aligned = new Table(16);
        for (int i = 0; i < 40; i++) {
            aligned.put(i * 1024, i);
        }
        int hits = 0;
        for (int i = 0; i < 40; i++) {
            if (aligned.get(i * 1024, -1) == i) {
                hits++;
            }
        }
        System.out.println("aligned keys: " + hits + " hits, capacity " + aligned.capacity());

        String[] words = words(TEXT);
        System.out.println("words: " + words.length);
        Table counts = new Table(8);
        String[] names = new String[words.length];
        countWords(words, counts, names);
        showTopWords(counts, words, 4);
        System.out.println("count of 'the': " + counts.get(hashWord("the"), 0));
        System.out.println("count of 'zebra': " + counts.get(hashWord("zebra"), 0));
        System.out.println("hash of 'cat': " + hashWord("cat"));
    }
}
