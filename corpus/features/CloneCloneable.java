public class CloneCloneable {
    static class Point implements Cloneable {
        int x;
        String label;
        int[] cells;
        Point inner;

        Point(int x, String label, int[] cells) {
            this.x = x;
            this.label = label;
            this.cells = cells;
        }

        // `super.clone()` copies the fields: the primitive and the string are copied, the array and
        // the object reference are shared by both objects.
        Point copy() throws CloneNotSupportedException {
            return (Point) super.clone();
        }

        // An unqualified `clone()` resolves to `Object`'s too.
        Point again() throws CloneNotSupportedException {
            return (Point) clone();
        }
    }

    static class Plain {
        int v;

        Plain(int v) {
            this.v = v;
        }
    }

    static class Holder implements Cloneable {
        Plain plain = new Plain(3);
        int count;

        Holder copy() throws CloneNotSupportedException {
            Holder h = (Holder) super.clone();
            h.count++;
            return h;
        }
    }

    public static void main(String[] args) throws CloneNotSupportedException {
        Point a = new Point(1, "a", new int[] {1, 2});
        a.inner = new Point(9, "in", new int[0]);
        Point b = a.copy();
        b.x = 5;
        b.label = "b";
        b.cells[0] = 7;
        b.inner.x = 10;
        System.out.println(a.x + " " + a.label + " " + a.cells[0] + " " + a.inner.x);
        System.out.println(b.x + " " + b.label + " " + b.cells[0] + " " + b.inner.x);
        System.out.println((b != a) + " " + (b.inner == a.inner) + " " + (b.cells == a.cells));

        b.inner = null;
        System.out.println((a.inner == null) + " " + (b.inner == null));

        Point c = b.again();
        System.out.println(c.x + " " + c.label + " " + c.cells[0] + " " + (c.inner == null));

        Holder h = new Holder();
        Holder g = h.copy();
        g.plain.v = 4;
        System.out.println(h.count + " " + g.count + " " + h.plain.v + " " + g.plain.v);
    }
}
