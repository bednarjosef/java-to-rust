package collections.util;

// What every collection of the project offers: its size, its elements by position, and adding.
public interface Sequence<T> {
    int size();

    T get(int i);

    void add(T t);

    default boolean isEmpty() {
        return size() == 0;
    }
}
