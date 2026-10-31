package java.util;

/**
 * The translator's patch of the JDK's {@code WeakHashMap} (see docs/jdk-overlays.md). The JDK's
 * {@code matchesKey} asks whether the entry refers to the key, by identity, before it reads the
 * referent, so as not to keep it strongly reachable; a string or a box has no identity that the
 * translation keeps, so the patched one reads the referent and tests it as
 * {@code Objects.equals} does, {@code k == key} before {@code key.equals(k)}, which gives the
 * same answer. The referent it reads is held only while the method runs.
 */
class WeakHashMap {
    @Replace
    private boolean matchesKey(Entry<K,V> e, Object key) {
        Object k = e.get();
        return k == key || k != null && key.equals(k);
    }
}
