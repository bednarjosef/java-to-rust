// Constructors that throw in chains of objects: a linked list whose node constructor checks
// the value and the next node (`next != null && next.value == value` needs no null check), an
// abstract class whose constructor throws for its subclasses' `super(...)`, and a subclass
// constructor that catches a division by zero and throws an exception with that cause.
public class ThrowingConstructorChains {
    static class Node {
        int value;
        Node next;

        Node(int value, Node next) {
            if (value < 0) {
                throw new IllegalArgumentException("negative " + value);
            }
            this.value = value;
            this.next = next;
            if (next != null && next.value == value) {
                throw new IllegalStateException("duplicate " + value);
            }
        }
    }

    static abstract class Shape {
        final String name;

        Shape(String name) {
            if (name.isEmpty()) {
                throw new IllegalArgumentException("no name");
            }
            this.name = name;
        }

        abstract int area();
    }

    static class Square extends Shape {
        final int side;

        Square(String name, int side) {
            super(name);
            int checked;
            try {
                checked = 100 / side;
            } catch (ArithmeticException e) {
                throw new IllegalArgumentException("side " + side, e);
            }
            this.side = side + checked * 0;
        }

        int area() {
            return side * side;
        }
    }

    static class Circle extends Shape {
        final int r;

        Circle(String name, int r) {
            super(name);
            this.r = r;
        }

        int area() {
            return 3 * r * r;
        }
    }

    public static void main(String[] args) {
        Node list = null;
        int[] values = {3, 5, 5, -1, 7};
        for (int v : values) {
            try {
                list = new Node(v, list);
            } catch (RuntimeException e) {
                System.out.println("skipped: " + e.getMessage());
            }
        }
        for (Node n = list; n != null; n = n.next) {
            System.out.print(n.value + " ");
        }
        System.out.println();
        Shape[] shapes = new Shape[4];
        int count = 0;
        String[] names = {"a", "", "c", "d"};
        int[] sizes = {2, 3, 0, 4};
        for (int i = 0; i < names.length; i++) {
            try {
                shapes[count] = i % 2 == 0 ? new Square(names[i], sizes[i]) : new Circle(names[i], sizes[i]);
                count++;
            } catch (IllegalArgumentException e) {
                System.out.println(names[i] + ": " + e.getMessage() + (e.getCause() != null ? " because " + e.getCause().getMessage() : ""));
            }
        }
        int total = 0;
        for (int i = 0; i < count; i++) {
            total += shapes[i].area();
            System.out.println(shapes[i].name + " " + shapes[i].area());
        }
        System.out.println(total);
    }
}
