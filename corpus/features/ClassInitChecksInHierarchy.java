// Classes initialized through a hierarchy, used in a loop: a subclass's initialization runs its
// superclass's and its interface's with code first, a `super(..)` initializes nothing, and the
// order the static blocks print in is Java's whichever check the translation leaves out.
public class ClassInitChecksInHierarchy {
    interface Shape {
        int SIDES = Log.note("Shape", 4);

        default int sides() {
            return SIDES;
        }
    }

    static class Log {
        static int notes;

        static int note(String who, int value) {
            notes++;
            System.out.println("init " + who + " (" + notes + ")");
            return value;
        }
    }

    static class Point {
        static int created;
        static final int[] OFFSETS;
        static int scale;

        static {
            OFFSETS = new int[] { 1, 2, 3 };
            scale = Log.note("Point", 2);
        }

        final int x;
        final int y;

        Point(int x, int y) {
            this.x = x;
            this.y = y;
            created++;
        }

        static int offset(int i) {
            return OFFSETS[i % OFFSETS.length] * scale;
        }

        int scaled() {
            return x * scale + y * scale;
        }
    }

    static class Cell extends Point implements Shape {
        static int cells;
        static final int LIMIT;

        static {
            LIMIT = Log.note("Cell", 1000);
        }

        Cell(int x, int y) {
            super(x, y);
            cells++;
        }

        Cell(int both) {
            this(both, both);
        }

        Cell next() {
            return new Cell(x + offset(x), y + 1);
        }

        int weight() {
            return scaled() + created % 7 + cells % 5 + sides();
        }

        static int clamp(int v) {
            return v % LIMIT;
        }
    }

    static class Grid {
        static int reads;

        static {
            Log.note("Grid", 0);
        }

        static int sum(int n) {
            reads++;
            int total = 0;
            Cell c = new Cell(1);
            for (int i = 0; i < n; i++) {
                c = c.next();
                total = Cell.clamp(total + c.weight());
                total += Point.offset(i) + Point.scale;
                Point p = new Point(i, Cell.cells);
                total = Cell.clamp(total + p.scaled());
            }
            return total;
        }
    }

    public static void main(String[] args) {
        System.out.println("start");
        Point first = new Point(1, 2);
        System.out.println("point " + first.scaled() + " created " + Point.created);
        System.out.println("sum " + Grid.sum(50));
        System.out.println("sum " + Grid.sum(200));
        System.out.println("cells " + Cell.cells + " created " + Point.created + " reads " + Grid.reads);
        System.out.println("notes " + Log.notes);
    }
}
