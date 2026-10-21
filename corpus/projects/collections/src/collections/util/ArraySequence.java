package collections.util;

// A sequence in an array that doubles when it is full.
public class ArraySequence<T> implements Sequence<T> {
    private Object[] items = new Object[2];
    private int size;

    @Override
    public int size() {
        return size;
    }

    @Override
    @SuppressWarnings("unchecked")
    public T get(int i) {
        return (T) items[i];
    }

    @Override
    public void add(T t) {
        if (size == items.length) {
            Object[] bigger = new Object[size * 2];
            for (int i = 0; i < size; i++) {
                bigger[i] = items[i];
            }
            items = bigger;
        }
        items[size++] = t;
    }

    public void set(int i, T t) {
        items[i] = t;
    }
}
