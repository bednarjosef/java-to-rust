// A class that is a type argument somewhere, as Box is of Stack<Box<String>>, is shared, not
// owned: a generic class clones its type arguments' values, and a plain owned value is not Clone.
public class OwnedTypeArguments {
    static class Box<T> {
        T val;
        Box(T val) {
            this.val = val;
        }
        T get() {
            return val;
        }
        void set(T v) {
            val = v;
        }
    }
    static class Node<T> {
        T val;
        Node<T> next;
        Node(T val, Node<T> next) {
            this.val = val;
            this.next = next;
        }
    }
    static class Stack<T> {
        Node<T> head;
        int size;
        void push(T v) {
            head = new Node<>(v, head);
            size++;
        }
        int size() {
            return size;
        }
    }

    public static void main(String[] args) {
        Stack<Box<String>> empty = new Stack<>();
        System.out.println(empty.size());
        Box<Integer> b = new Box<>(4);
        b.set(b.get() * 3);
        System.out.println(b.get() + 1);
        Stack<Box<Integer>> full = new Stack<>();
        System.out.println(full.size() + " " + (full.head == null));
    }
}
