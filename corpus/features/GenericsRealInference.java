// Rust generics instantiated with the types javac inferred: a constructor whose arguments do
// not decide the type arguments (none, or only null) names them, and an argument of a subclass
// is converted to the substituted type (a `Box<Shape>` given a `Square`, `pick` of a line and a
// square inferred as a `Shape`).
public class GenericsRealInference {
    static class Box<T> {
        T val;
        Box(T val) { this.val = val; }
        T get() { return val; }
        void set(T v) { val = v; }
    }

    static class Node<T> {
        T val;
        Node<T> next;
        Node(T val, Node<T> next) { this.val = val; this.next = next; }
    }

    static class Stack<T> {
        Node<T> head;
        int size;
        void push(T v) { head = new Node<>(v, head); size++; }
        T pop() { T v = head.val; head = head.next; size--; return v; }
        boolean isEmpty() { return head == null; }
    }

    static abstract class Shape {
        int n;
        Shape(int n) { this.n = n; }
        abstract int area();
    }

    static class Square extends Shape {
        Square(int n) { super(n); }
        int area() { return n * n; }
    }

    static class Line extends Shape {
        Line(int n) { super(n); }
        int area() { return 0; }
    }

    static <T> T pick(boolean c, T a, T b) { return c ? a : b; }

    public static void main(String[] args) {
        Box<String> empty = new Box<>(null);
        System.out.println(empty.get() == null);
        empty.set("full");
        System.out.println(empty.get());
        Stack<String> words = new Stack<>();
        System.out.println(words.isEmpty());
        Stack<Integer> numbers = new Stack<>();
        for (int i = 0; i < 3 && !numbers.isEmpty(); i++) {
            System.out.println(numbers.pop());
        }
        numbers.push(4);
        Integer top = numbers.pop();
        System.out.println(top + 1);

        Box<Shape> b = new Box<>(new Square(2));
        System.out.println(b.get().area());
        b.set(new Line(3));
        System.out.println(b.get().area() + " " + b.get().n);
        Square q = new Square(5);
        b.set(q);
        Shape s = pick(args.length == 0, new Line(1), q);
        System.out.println(s.area() + " " + b.get().area());
        Shape t = pick(args.length != 0, s, new Square(7));
        System.out.println(t.area());
        Stack<Shape> shapes = new Stack<>();
        shapes.push(q);
        shapes.push(new Line(8));
        System.out.println(shapes.pop().n + shapes.pop().area());
    }
}
