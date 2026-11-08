package java.lang;

import java.util.concurrent.atomic.AtomicLong;

/**
 * The translator's patch of the JDK's {@code ThreadLocal} (see docs/jdk-overlays.md and
 * docs/multi-threading.md, "Thread-locals by slot number"): a thread-local's value is found by a
 * slot number of its own instead of by hashing. Each of the first {@code DENSE_SLOTS}
 * thread-locals made gets the next number, which no other thread-local ever gets, and each
 * thread's map keeps, besides its hash table, an array from those numbers to the table's entries. {@code get} and {@code set} of a
 * thread-local with a number read that array: an index, a bounds check and a load, where the
 * table needs a hash, a probe and a test of the entry's weak referent.
 *
 * <p>The table stays what decides: an entry is put in the array where it is put in the table, and
 * taken out wherever it leaves the table (removed, found stale, or dropped when the table grows),
 * so the array holds exactly the table's entries of numbered thread-locals and keeps nothing alive
 * that the table does not. A number is never given again, so an entry found at a live
 * thread-local's number is that thread-local's: its weak referent, cleared only once the
 * thread-local is unreachable or by {@code remove}, which takes the entry out, needs no test. The
 * array is the map's, so whatever replaces or drops a thread's map (inheritance, a thread's exit,
 * the {@code Unsafe} erasing of a pool thread's thread-locals) does the same to the array.
 */
class ThreadLocal {
    /** How many thread-locals get a slot number; later ones are found by hashing alone. */
    @Add
    private static final int DENSE_SLOTS = 4096;

    /** The thread-local's slot number, or -1 when all are taken. */
    @Add
    private final int denseSlot = nextDenseSlot();

    /**
     * The next slot number, counted where no wrap-around can give a number twice. The counter is
     * in a class of its own because the JDK's static initializer makes a thread-local before an
     * added static field of this class would be set.
     */
    @Add
    private static int nextDenseSlot() {
        long slot = DenseSlots.NEXT.getAndIncrement();
        return slot < DENSE_SLOTS ? (int) slot : -1;
    }

    @Add
    private static final class DenseSlots {
        static final AtomicLong NEXT = new AtomicLong();
    }

    @Patch
    static class ThreadLocalMap {
        @Patch
        static class Entry {
            /** The slot number of the entry's thread-local, kept for when the referent is cleared. */
            @Add
            final int slot;

            @Edit(find = "value = v;", replace = """
                    value = v;
                    slot = k.denseSlot;
                    """)
            Entry(ThreadLocal<?> k, Object v);
        }

        /** The entries of numbered thread-locals by number; as long as the highest one held. */
        @Add
        private Entry[] dense = NO_ENTRIES;

        @Add
        private static final Entry[] NO_ENTRIES = new Entry[0];

        /** Puts an entry just put in the table into the array. */
        @Add
        private void remember(Entry e) {
            int s = e.slot;
            if (s < 0) {
                return;
            }
            Entry[] d = dense;
            if (s >= d.length) {
                int n = Math.max(d.length, INITIAL_CAPACITY);
                while (n <= s) {
                    n <<= 1;
                }
                Entry[] grown = new Entry[n];
                System.arraycopy(d, 0, grown, 0, d.length);
                dense = d = grown;
            }
            d[s] = e;
        }

        /** Takes an entry that leaves the table out of the array. */
        @Add
        private void forget(Entry e) {
            int s = e.slot;
            Entry[] d = dense;
            if (s >= 0 && s < d.length && d[s] == e) {
                d[s] = null;
            }
        }

        @Edit(find = "table[i] = new Entry(firstKey, firstValue);", replace = "remember(table[i] = new Entry(firstKey, firstValue));")
        ThreadLocalMap(ThreadLocal<?> firstKey, Object firstValue);

        @Edit(find = "table[h] = c;", replace = """
                table[h] = c;
                remember(c);
                """)
        private ThreadLocalMap(ThreadLocalMap parentMap);

        @Edit(find = "int i = key.threadLocalHashCode & (table.length - 1);", replace = """
                int s = key.denseSlot;
                if (s >= 0) {
                    Entry[] d = dense;
                    return s < d.length ? d[s] : null;
                }
                int i = key.threadLocalHashCode & (table.length - 1);
                """)
        private Entry getEntry(ThreadLocal<?> key);

        @Edit(find = "Entry[] tab = table;", replace = """
                int s = key.denseSlot;
                if (s >= 0) {
                    Entry[] d = dense;
                    Entry found = s < d.length ? d[s] : null;
                    if (found != null) {
                        found.value = value;
                        return;
                    }
                }
                Entry[] tab = table;
                """)
        @Edit(find = "tab[i] = new Entry(key, value);", replace = "remember(tab[i] = new Entry(key, value));")
        private void set(ThreadLocal<?> key, Object value);

        @Edit(find = "tab[staleSlot].value = null;", replace = """
                forget(tab[staleSlot]);
                tab[staleSlot].value = null;
                """)
        @Edit(find = "tab[staleSlot] = new Entry(key, value);", replace = "remember(tab[staleSlot] = new Entry(key, value));")
        private void replaceStaleEntry(ThreadLocal<?> key, Object value, int staleSlot);

        @Edit(find = "tab[staleSlot].value = null;", replace = """
                forget(tab[staleSlot]);
                tab[staleSlot].value = null;
                """)
        @Edit(find = "e.value = null;", replace = """
                forget(e);
                e.value = null;
                """)
        private int expungeStaleEntry(int staleSlot);

        @Edit(find = "e.value = null;", replace = "forget(e); e.value = null;")
        private void resize();
    }
}
