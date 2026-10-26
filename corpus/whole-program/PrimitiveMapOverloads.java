import java.util.Collection;
import java.util.HashMap;
import java.util.Map;
import java.util.Set;

// Maps of primitive values shaped as fastutil's `Reference2FloatMap`: the function interface's
// `put(K, float)` and `Map`'s `put(K, V)` are each alone in the class trees that declare them,
// and only the map interface, which extends both, brings them together, so its enum holds a
// dispatcher for each.
public class PrimitiveMapOverloads {
    interface FloatFunction<K> {
        default float put(K key, float value) {
            throw new UnsupportedOperationException();
        }

        float getFloat(Object key);
    }

    interface FloatMap<K> extends FloatFunction<K>, Map<K, Float> {
        @Override
        default Float put(K key, Float value) {
            boolean present = containsKey(key);
            float old = put(key, value.floatValue());
            return present ? Float.valueOf(old) : null;
        }
    }

    abstract static class AbstractFloatFunction<K> implements FloatFunction<K> {
        @Override
        public float getFloat(Object key) {
            Float value = get(key);
            return value == null ? -1f : value;
        }

        abstract Float get(Object key);
    }

    static class FloatHashMap<K> extends AbstractFloatFunction<K> implements FloatMap<K> {
        final HashMap<K, Float> entries = new HashMap<>();

        @Override
        public float put(K key, float value) {
            Float old = entries.put(key, value);
            return old == null ? -1f : old;
        }

        @Override
        public Float get(Object key) {
            return entries.get(key);
        }

        @Override
        public int size() {
            return entries.size();
        }

        @Override
        public boolean isEmpty() {
            return entries.isEmpty();
        }

        @Override
        public boolean containsKey(Object key) {
            return entries.containsKey(key);
        }

        @Override
        public boolean containsValue(Object value) {
            return entries.containsValue(value);
        }

        @Override
        public Float remove(Object key) {
            return entries.remove(key);
        }

        @Override
        public void putAll(Map<? extends K, ? extends Float> other) {
            entries.putAll(other);
        }

        @Override
        public void clear() {
            entries.clear();
        }

        @Override
        public Set<K> keySet() {
            return entries.keySet();
        }

        @Override
        public Collection<Float> values() {
            return entries.values();
        }

        @Override
        public Set<Map.Entry<K, Float>> entrySet() {
            return entries.entrySet();
        }
    }

    // A second map, so that a `FloatMap` may be either.
    static class DoublingMap<K> extends FloatHashMap<K> {
        @Override
        public float put(K key, float value) {
            return super.put(key, value * 2);
        }
    }

    public static void main(String[] args) {
        FloatMap<String> plain = new FloatHashMap<>();
        FloatMap<String> doubling = new DoublingMap<>();
        for (FloatMap<String> weights : java.util.List.of(plain, doubling)) {
            System.out.println(weights.put("stone", 1.5f));
            System.out.println(weights.put("stone", 2.5f));
            System.out.println(weights.put("dirt", Float.valueOf(0.5f)));
            Map<String, Float> map = weights;
            System.out.println(map.put("sand", 3f));
            System.out.println(map.get("sand") + " " + weights.getFloat("dirt") + " " + weights.getFloat("none"));
            System.out.println(weights.size());
        }
    }
}
