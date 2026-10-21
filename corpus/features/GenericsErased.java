// Generic classes and methods, erased: bounded type variables, wildcards, a subclass of a
// parameterized class, a generic array built from `Object[]`, and explicit type arguments.
public class GenericsErased {
    static abstract class Shape {
        abstract double area();
    }

    static class Square extends Shape {
        final double side;
        Square(double side) { this.side = side; }
        double area() { return side * side; }
    }

    static class Circle extends Shape {
        final double r;
        Circle(double r) { this.r = r; }
        double area() { return 3 * r * r; }
    }

    static class Box<T> {
        T value;
        Box(T value) { this.value = value; }
        T get() { return value; }
        <R> Box<R> map(Mapper<T, R> f) { return new Box<>(f.apply(value)); }
    }

    interface Mapper<A, B> {
        B apply(A a);
    }

    static class Doubler implements Mapper<Integer, Integer> {
        public Integer apply(Integer x) { return x * 2; }
    }

    static class Show implements Mapper<Integer, String> {
        public String apply(Integer x) { return "#" + x; }
    }

    static class IntBox extends Box<Integer> {
        IntBox(int v) { super(v); }
        @Override Integer get() { return value + 100; }
    }

    static class Stack<T> {
        private T[] items;
        private int size;
        @SuppressWarnings("unchecked")
        Stack(int capacity) { items = (T[]) new Object[capacity]; }
        void push(T t) { items[size++] = t; }
        T pop() { T t = items[--size]; items[size] = null; return t; }
        boolean isEmpty() { return size == 0; }
    }

    static <S extends Shape> double total(Stack<S> shapes) {
        double sum = 0;
        while (!shapes.isEmpty()) sum += shapes.pop().area();
        return sum;
    }

    static double sumAreas(Box<? extends Shape> a, Box<? extends Shape> b) {
        return a.get().area() + b.get().area();
    }

    static <T> Box<T> boxed(T t) { return new Box<>(t); }

    public static void main(String[] args) {
        Box<Integer> b = new Box<>(21);
        System.out.println(b.map(new Doubler()).get() + " " + b.map(new Show()).get());
        Box<Integer> ib = new IntBox(5);
        System.out.println(ib.get() + " " + new IntBox(1).get());
        Stack<Shape> shapes = new Stack<>(4);
        shapes.push(new Square(2));
        shapes.push(new Circle(1));
        System.out.println(total(shapes));
        System.out.println(sumAreas(new Box<>(new Square(3)), new Box<Circle>(new Circle(2))));
        Box<Box<String>> nested = boxed(boxed("deep"));
        System.out.println(nested.get().get().toUpperCase());
        long big = GenericsErased.<Long>boxed(5L).get() * 3;
        System.out.println(big);
    }
}
