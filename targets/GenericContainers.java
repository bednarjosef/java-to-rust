// Features: user generic classes Pair<A, B>, Box<T>, and a generic linked Stack<T> with an inner
// Node class, generic methods with explicit and inferred type arguments, a custom unchecked
// exception on an empty stack, static nested generic class, equals and hashCode in user code.
import java.util.Objects;

public class GenericContainers {
    static final class Pair<A, B> {
        private final A first;
        private final B second;

        Pair(A first, B second) {
            this.first = first;
            this.second = second;
        }

        A getFirst() {
            return first;
        }

        B getSecond() {
            return second;
        }

        Pair<B, A> swap() {
            return new Pair<>(second, first);
        }

        @Override
        public boolean equals(Object o) {
            if (!(o instanceof Pair)) {
                return false;
            }
            Pair<?, ?> other = (Pair<?, ?>) o;
            return Objects.equals(first, other.first) && Objects.equals(second, other.second);
        }

        @Override
        public int hashCode() {
            return Objects.hash(first, second);
        }

        @Override
        public String toString() {
            return "(" + first + ", " + second + ")";
        }
    }

    static class Box<T> {
        private T value;

        Box(T value) {
            this.value = value;
        }

        T get() {
            return value;
        }

        void set(T value) {
            this.value = value;
        }

        <R> Box<R> map(java.util.function.Function<T, R> f) {
            return new Box<>(f.apply(value));
        }
    }

    static class EmptyStackException extends RuntimeException {
        EmptyStackException(String message) {
            super(message);
        }
    }

    static class Stack<T> implements Iterable<T> {
        private static class Node<T> {
            final T item;
            final Node<T> next;

            Node(T item, Node<T> next) {
                this.item = item;
                this.next = next;
            }
        }

        private Node<T> top;
        private int size;

        void push(T item) {
            top = new Node<>(item, top);
            size++;
        }

        T pop() {
            if (top == null) {
                throw new EmptyStackException("pop from empty stack");
            }
            T item = top.item;
            top = top.next;
            size--;
            return item;
        }

        T peek() {
            if (top == null) {
                throw new EmptyStackException("peek at empty stack");
            }
            return top.item;
        }

        boolean isEmpty() {
            return size == 0;
        }

        int size() {
            return size;
        }

        @Override
        public java.util.Iterator<T> iterator() {
            return new java.util.Iterator<T>() {
                private Node<T> cursor = top;

                @Override
                public boolean hasNext() {
                    return cursor != null;
                }

                @Override
                public T next() {
                    T item = cursor.item;
                    cursor = cursor.next;
                    return item;
                }
            };
        }
    }

    static <K, V> Pair<K, V> entry(K key, V value) {
        return new Pair<>(key, value);
    }

    @SafeVarargs
    static <T> void pushAll(Stack<T> stack, T... items) {
        for (T item : items) {
            stack.push(item);
        }
    }

    public static void main(String[] args) {
        Pair<String, Integer> age = new Pair<>("ada", 36);
        System.out.println(age + " swapped " + age.swap());
        System.out.println("equal pairs: " + age.equals(entry("ada", 36)) + " " + age.equals(entry("ada", 37)));
        System.out.println("hash equal: " + (age.hashCode() == entry("ada", 36).hashCode()));

        Box<String> word = new Box<>("generic");
        Box<Integer> length = word.map(String::length);
        word.set("boxes");
        System.out.println("box " + word.get() + " length " + length.get());

        Stack<String> names = new Stack<>();
        pushAll(names, "x", "y", "z");
        System.out.println("size " + names.size() + " peek " + names.peek());
        StringBuilder sb = new StringBuilder();
        for (String s : names) {
            sb.append(s);
        }
        System.out.println("iterated " + sb);
        while (!names.isEmpty()) {
            System.out.print(names.pop() + " ");
        }
        System.out.println();
        try {
            names.pop();
        } catch (EmptyStackException e) {
            System.out.println("caught: " + e.getMessage());
        }

        Stack<Pair<Integer, Integer>> points = new Stack<>();
        for (int i = 0; i < 4; i++) {
            points.push(new Pair<>(i, i * i));
        }
        int total = 0;
        for (Pair<Integer, Integer> p : points) {
            total += p.getSecond() - p.getFirst();
        }
        System.out.println("points " + points.peek() + " total " + total);
        System.out.println("swap of swap: " + points.pop().swap().swap());
    }
}
