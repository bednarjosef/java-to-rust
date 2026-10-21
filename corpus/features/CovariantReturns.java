public class CovariantReturns {
    static abstract class Shape {
        abstract String name();

        abstract Shape twin();
    }

    static class Circle extends Shape {
        final int r;

        Circle(int r) {
            this.r = r;
        }

        String name() {
            return "circle " + r;
        }

        // Covariant: callers through Shape see a Shape, callers through Circle see a Circle.
        Circle twin() {
            return new Circle(r + 1);
        }
    }

    static class Square extends Shape {
        final int side;

        Square(int side) {
            this.side = side;
        }

        String name() {
            return "square " + side;
        }

        Square twin() {
            return new Square(side * 2);
        }
    }

    // A chain: Mid is a supertype of Leaf and has its own covariant override; Leaf's override
    // returns an object, Mid's an enum value that converts to Base's enum.
    static class Base {
        int id() {
            return 0;
        }

        Base next() {
            return new Base();
        }
    }

    static class Mid extends Base {
        int id() {
            return 1;
        }

        Mid next() {
            return new Mid();
        }
    }

    static class Leaf extends Mid {
        final int v;

        Leaf(int v) {
            this.v = v;
        }

        int id() {
            return v;
        }

        Leaf next() {
            return new Leaf(v + 1);
        }
    }

    public static void main(String[] args) {
        Shape[] shapes = {new Circle(1), new Square(3)};
        for (Shape s : shapes) {
            Shape t = s.twin();
            System.out.println(s.name() + " -> " + t.name());
        }
        Circle c = new Circle(5);
        Circle ct = c.twin();
        System.out.println(ct.name() + " " + ct.twin().twin().r);
        Square sq = new Square(4);
        System.out.println(sq.twin().twin().name());
        Shape viaShape = sq.twin();
        System.out.println(viaShape.twin().name());

        Base b = new Leaf(7);
        System.out.println(b.next().id() + " " + b.next().next().id());
        Base m = new Mid();
        Base chain = m.next();
        System.out.println(chain.id() + " " + chain.next().id());
        Mid mid = new Mid();
        Mid mm = mid.next();
        System.out.println(mm.id());
        Leaf lf = new Leaf(2);
        Mid asMid = lf.next();
        System.out.println(asMid.id() + " " + asMid.next().id());
    }
}
