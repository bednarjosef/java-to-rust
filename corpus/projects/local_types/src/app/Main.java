package app;

public class Main {
    static int base = 10;

    static int run(int k) {
        record Point(int x, int y) {
            int sum() { return x + y; }
        }
        enum Color { RED, GREEN; Color next() { return values()[(ordinal() + 1) % 2]; } }
        interface Shape { double area(); }
        Shape square = () -> 4.0;
        Point p = new Point(k, base);
        System.out.println(p + " " + p.sum() + " " + Color.RED.next() + " " + square.area());
        System.out.println(p.getClass().getName() + " " + p.getClass().getSimpleName());
        System.out.println(Shape.class.getName() + " " + Shape.class.getSimpleName());
        return p.sum();
    }

    public static void main(String[] args) {
        System.out.println(run(3));
        System.out.println(new Point().toString());
    }
}
