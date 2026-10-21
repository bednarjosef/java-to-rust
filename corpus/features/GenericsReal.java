// Generic classes and methods that become Rust generics: a linked stack, a pair whose swap
// changes its type arguments, a cell that may hold null, nested instantiations, and generic
// methods whose type parameters Rust infers from their arguments.
public class GenericsReal {
    static class Node<T> {
        T value;
        Node<T> next;
        Node(T value, Node<T> next) { this.value = value; this.next = next; }
    }

    static class LinkedStack<T> {
        private Node<T> head;
        private int size;

        void push(T value) {
            head = new Node<>(value, head);
            size++;
        }

        T pop() {
            if (head == null) return null;
            T value = head.value;
            head = head.next;
            size--;
            return value;
        }

        T peek() { return head == null ? null : head.value; }
        boolean isEmpty() { return head == null; }
        int size() { return size; }
    }

    static class Pair<A, B> {
        final A first;
        final B second;
        Pair(A first, B second) { this.first = first; this.second = second; }
        A first() { return first; }
        B second() { return second; }
        Pair<B, A> swap() { return new Pair<>(second, first); }
    }

    static class Point {
        int x, y;
        Point(int x, int y) { this.x = x; this.y = y; }
    }

    static class Cell<T> {
        private T value;
        boolean isSet() { return value != null; }
        void set(T v) { value = v; }
        T get() { return value; }
    }

    static <T> T choose(boolean first, T a, T b) { return first ? a : b; }

    static <A, B> Pair<A, B> pair(A a, B b) { return new Pair<>(a, b); }

    public static void main(String[] args) {
        LinkedStack<String> words = new LinkedStack<>();
        words.push("a");
        words.push("bb");
        words.push("ccc");
        System.out.println(words.size() + " " + words.peek() + " " + words.pop() + " " + words.pop().length() + " " + words.size());
        LinkedStack<Integer> numbers = new LinkedStack<>();
        for (int i = 0; i < 5; i++) numbers.push(i * i);
        int sum = 0;
        while (!numbers.isEmpty()) sum += numbers.pop();
        System.out.println(sum + " " + numbers.pop() + " " + numbers.peek());
        LinkedStack<Point> points = new LinkedStack<>();
        points.push(new Point(1, 2));
        points.peek().x = 10;
        System.out.println(points.pop().x);

        Pair<String, Integer> p = new Pair<>("one", 1);
        Pair<Integer, String> q = p.swap();
        System.out.println(p.first() + p.second() + " " + q.first() + q.second() + " " + (q.first() + 1));
        Pair<String, Pair<Integer, Double>> nested = pair("n", pair(2, 2.5));
        System.out.println(nested.second().second() * 2 + " " + nested.first().toUpperCase());

        Cell<String> cell = new Cell<>();
        System.out.println(cell.isSet() + " " + cell.get());
        cell.set("x");
        System.out.println(cell.isSet() + " " + cell.get());

        System.out.println(choose(true, "left", "right") + " " + choose(false, 1, 2) + " " + choose(true, 1.5, 2.5));
        String s = choose(false, "a", null);
        System.out.println(s == null);
    }
}
