package java.util;

/**
 * The translator's patch of the JDK's {@code WeakHashMap} (see docs/jdk-overlays.md). The JDK's
 * {@code matchesKey} asks whether the entry refers to the key before it reads the referent, so as
 * not to keep it strongly reachable; a translated program has no collector that clears a
 * reference, so the patched one reads the referent and tests it as {@code Objects.equals} does,
 * {@code k == key} before {@code key.equals(k)}, which gives the same answer when the key is a
 * string or a box, whose identity the translation does not keep.
 */
class WeakHashMap {
    @Replace
    private boolean matchesKey(Entry<K,V> e, Object key) {
        Object k = e.get();
        return k == key || k != null && key.equals(k);
    }
}
