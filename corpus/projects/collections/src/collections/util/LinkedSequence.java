package collections.util;

// A sequence of linked nodes, appended at its tail.
public class LinkedSequence<T> implements Sequence<T> {
    private static class Node<T> {
        final T value;
        Node<T> next;

        Node(T value) {
            this.value = value;
        }
    }

    private Node<T> head;
    private Node<T> tail;
    private int size;

    @Override
    public int size() {
        return size;
    }

    @Override
    public T get(int i) {
        Node<T> n = head;
        for (int k = 0; k < i; k++) {
            n = n.next;
        }
        return n.value;
    }

    @Override
    public void add(T t) {
        Node<T> n = new Node<>(t);
        if (tail == null) {
            head = n;
        } else {
            tail.next = n;
        }
        tail = n;
        size++;
    }
}
