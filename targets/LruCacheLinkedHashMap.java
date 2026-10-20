// Features: a generic LruCache<K, V> that extends LinkedHashMap with access order and overrides
// removeEldestEntry (anonymous subclass constructor argument and a named subclass), a generic
// Pair holder, Map methods getOrDefault, computeIfAbsent, putIfAbsent, remove and containsKey,
// an eviction listener as a functional interface, an inner class counting hits and misses, a
// memoizing Fibonacci through the cache, and iteration over entrySet in access order.
import java.util.LinkedHashMap;
import java.util.Map;

public class LruCacheLinkedHashMap {
    interface EvictionListener<K, V> {
        void evicted(K key, V value);
    }

    static class LruCache<K, V> extends LinkedHashMap<K, V> {
        private final int capacity;
        private final EvictionListener<K, V> listener;

        LruCache(int capacity, EvictionListener<K, V> listener) {
            super(16, 0.75f, true);
            this.capacity = capacity;
            this.listener = listener;
        }

        @Override
        protected boolean removeEldestEntry(Map.Entry<K, V> eldest) {
            boolean evict = size() > capacity;
            if (evict) {
                listener.evicted(eldest.getKey(), eldest.getValue());
            }
            return evict;
        }
    }

    static class Stats {
        private int hits;
        private int misses;

        class Ratio {
            String percent() {
                int total = hits + misses;
                if (total == 0) {
                    return "n/a";
                }
                return (100 * hits / total) + "%";
            }
        }

        void hit() {
            hits++;
        }

        void miss() {
            misses++;
        }

        String summary() {
            return "hits=" + hits + " misses=" + misses + " ratio=" + new Ratio().percent();
        }
    }

    static class Pair<A, B> {
        final A first;
        final B second;

        Pair(A first, B second) {
            this.first = first;
            this.second = second;
        }

        @Override
        public String toString() {
            return "(" + first + ", " + second + ")";
        }
    }

    static final Stats stats = new Stats();
    static final LruCache<Integer, Long> fibCache = new LruCache<>(64, (k, v) -> { });

    static long fib(int n) {
        if (n < 2) {
            return n;
        }
        Long cached = fibCache.get(n);
        if (cached != null) {
            stats.hit();
            return cached;
        }
        stats.miss();
        long value = fib(n - 1) + fib(n - 2);
        fibCache.put(n, value);
        return value;
    }

    public static void main(String[] args) {
        StringBuilder evictions = new StringBuilder();
        LruCache<String, Integer> cache = new LruCache<>(3, (k, v) -> evictions.append(k).append('=').append(v).append(' '));
        cache.put("a", 1);
        cache.put("b", 2);
        cache.put("c", 3);
        System.out.println("initial: " + cache);
        System.out.println("get a: " + cache.get("a"));
        cache.put("d", 4);
        System.out.println("after put d: " + cache + " evicted: " + evictions.toString().trim());
        System.out.println("get missing: " + cache.get("zzz") + ", default: " + cache.getOrDefault("zzz", -1));
        cache.computeIfAbsent("e", k -> k.length() * 10);
        cache.putIfAbsent("e", 999);
        System.out.println("after e: " + cache + " evicted: " + evictions.toString().trim());
        System.out.println("containsKey c: " + cache.containsKey("c") + ", remove d: " + cache.remove("d"));
        cache.put("f", 6);
        cache.put("g", 7);
        for (Map.Entry<String, Integer> e : cache.entrySet()) {
            System.out.println("  entry " + e.getKey() + " -> " + e.getValue());
        }

        System.out.println("fib(40) = " + fib(40));
        System.out.println("fib(90) = " + fib(90));
        System.out.println("cache stats: " + stats.summary() + ", cached values " + fibCache.size());

        Pair<String, Integer> pair = new Pair<>("entries", cache.size());
        System.out.println("pair: " + pair);
    }
}
