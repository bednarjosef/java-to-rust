public class LocalClasses {
    interface Shape {
        double area();
        String name();
    }

    static class Counter {
        int count;
        void bump(int by) { count += by; }
    }

    static abstract class Greeter {
        final String greeting;
        Greeter(String greeting) { this.greeting = greeting; }
        abstract String target();
        String greet() { return greeting + ", " + target() + "!"; }
    }

    // Captures of every kind: an int, a string, an object, an array the class writes, and a
    // parameter.
    static void captures(int base, String unit) {
        int scale = base * 2;
        String prefix = "item-";
        Counter counter = new Counter();
        int[] seen = new int[3];
        class Labeler {
            int calls;
            String label(int n) {
                calls++;
                counter.bump(n);
                seen[n % 3] += 1;
                return prefix + (n * scale) + unit;
            }
        }
        Labeler labeler = new Labeler();
        for (int i = 0; i < 5; i++) {
            System.out.println(labeler.label(i));
        }
        System.out.println("calls " + labeler.calls + ", counted " + counter.count);
        System.out.println("seen " + seen[0] + " " + seen[1] + " " + seen[2]);
    }

    // Two local classes of one name in two methods.
    static String first(int x) {
        class Box {
            int value = x + 1;
            String show() { return "first box " + value; }
        }
        return new Box().show();
    }

    static String second(String s) {
        class Box {
            String show() { return "second box " + s.length(); }
        }
        return new Box().show();
    }

    // A local class extending a program class, and one implementing a program interface.
    static void hierarchy(String who, double side) {
        class Named extends Greeter {
            Named() { super("Hello"); }
            String target() { return who; }
        }
        class Square implements Shape {
            public double area() { return side * side; }
            public String name() { return "square of " + side; }
        }
        Greeter g = new Named();
        System.out.println(g.greet());
        Shape s = new Square();
        System.out.println(s.name() + " has area " + s.area());
    }

    // A local class without captures, created in a loop, with a constructor of its own.
    static int sumOfPairs(int n) {
        class Pair {
            final int a;
            final int b;
            Pair(int a, int b) { this.a = a; this.b = b; }
            int sum() { return a + b; }
        }
        int total = 0;
        for (int i = 0; i < n; i++) {
            Pair p = new Pair(i, i * i);
            total += p.sum();
        }
        return total;
    }

    // A local class in an instance method, which reads the enclosing object's fields.
    int factor = 3;
    String tag = "obj";

    String scaled(int v) {
        class Scaler {
            int apply() { return v * factor; }
            String describe() { return tag + ":" + apply(); }
        }
        Scaler s = new Scaler();
        factor++;
        return s.describe();
    }

    public static void main(String[] args) {
        captures(3, "kg");
        System.out.println(first(41));
        System.out.println(second("four"));
        hierarchy("world", 1.5);
        System.out.println(sumOfPairs(6));
        LocalClasses o = new LocalClasses();
        System.out.println(o.scaled(7));
        System.out.println(o.scaled(2));
    }
}
