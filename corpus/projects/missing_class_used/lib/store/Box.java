package store;

public class Box<T> {
    T item;

    int size() {
        return item == null ? 0 : 1;
    }
}
