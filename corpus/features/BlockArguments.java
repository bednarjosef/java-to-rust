// Arguments whose translation is a block of statements (an assignment's value, a static read
// that first runs the class's initializer) in calls whose borrows are scoped to the statement.
public class BlockArguments {
    static int counter = 7;
    static int[] values = {1, 2, 3};

    interface Shape {
        int size(int k);
    }

    static class Base implements Shape {
        int n = 2;
        Base link;

        public int size(int k) {
            n++;
            return k + n + (link != null ? link.n : -1);
        }
    }

    static class Square extends Base {
        public int size(int k) {
            int r = super.size((k %= (1234567890123L | 1))) + n;
            return r + super.size(values.length) + super.size((counter += 3));
        }
    }

    static class Line extends Base {
        public int size(int k) {
            return n * k;
        }
    }

    public static void main(String[] args) {
        Shape a = new Square();
        Shape b = new Line();
        Base c = new Square();
        c.link = (Base) a;
        System.out.println(a.size(100) + " " + b.size(4) + " " + c.size(9));
        System.out.println(counter);
    }
}
