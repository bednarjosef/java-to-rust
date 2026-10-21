// Pattern matching in switch: type and record patterns (nested), guards, `case null`, constants
// among patterns, exhaustive switches over sealed interfaces, a `break` out of a pattern case,
// and the NullPointerException of a null selector.
public class PatternSwitch {
    sealed interface Shape permits Circle, Square, Rect {}

    record Circle(double r) implements Shape {}

    record Square(double side) implements Shape {}

    record Rect(double w, double h) implements Shape {}

    record Point(int x, int y) {}

    record Line(Point from, Point to) {}

    sealed interface Expr permits Num, Add, Mul, Neg {}

    record Num(int value) implements Expr {}

    record Add(Expr left, Expr right) implements Expr {}

    record Mul(Expr left, Expr right) implements Expr {}

    record Neg(Expr inner) implements Expr {}

    enum Color { RED, GREEN, BLUE }

    static double area(Shape s) {
        return switch (s) {
            case Circle c -> Math.PI * c.r() * c.r();
            case Square q -> q.side() * q.side();
            case Rect(double w, double h) -> w * h;
        };
    }

    static String size(Shape s) {
        switch (s) {
            case Circle c when c.r() > 10:
                return "big circle";
            case Circle c:
                return "circle";
            case Rect(double w, double h) when w == h:
                return "square rect";
            default:
                return "other";
        }
    }

    static int eval(Expr e) {
        return switch (e) {
            case Num n -> n.value();
            case Add(Expr l, Expr r) -> eval(l) + eval(r);
            case Mul(Num(var a), Num(var b)) -> a * b;
            case Mul(Expr l, Expr r) -> eval(l) * eval(r);
            case Neg(Neg(Expr inner)) -> eval(inner);
            case Neg(Expr inner) -> -eval(inner);
        };
    }

    static String show(Expr e) {
        return switch (e) {
            case Num(int v) when v < 0 -> "(" + v + ")";
            case Num(int v) -> String.valueOf(v);
            case Add a -> "(" + show(a.left()) + " + " + show(a.right()) + ")";
            case Mul m -> show(m.left()) + " * " + show(m.right());
            case Neg n -> "-" + show(n.inner());
        };
    }

    static String describe(Object o) {
        return switch (o) {
            case null -> "null";
            case Integer i when i > 100 -> "large int " + i;
            case Integer i -> "int " + i;
            case String s when s.isEmpty() -> "empty string";
            case String s -> "string of " + s.length();
            case Line(Point(var x1, var y1), Point p) when x1 == p.x() -> "vertical from " + y1 + " to " + p.y();
            case Line l -> "line " + l;
            case Color c -> "color " + c.ordinal();
            default -> "something else";
        };
    }

    static String classify(String s) {
        switch (s) {
            case null -> {
                return "no text";
            }
            case "yes", "y" -> {
                return "agreed";
            }
            case String t when t.length() > 5 -> {
                return "long " + t;
            }
            default -> {
                return "other " + s;
            }
        }
    }

    static int weight(Color c) {
        int w = 0;
        switch (c) {
            case RED:
                w = 1;
                break;
            case Color k when k.ordinal() == 1:
                w = 10;
                break;
            default:
                w = 100;
        }
        return w;
    }

    static String firstBig(Shape[] shapes) {
        String found = "none";
        for (Shape s : shapes) {
            switch (s) {
                case Circle c when c.r() > 2 -> {
                    found = "circle " + c.r();
                }
                case Square q -> {
                    if (q.side() > 3) {
                        found = "square " + q.side();
                        break;
                    }
                    found = "small square";
                }
                default -> {}
            }
        }
        return found;
    }

    public static void main(String[] args) {
        Shape[] shapes = {new Circle(1), new Square(2), new Rect(2, 3), new Circle(11), new Rect(4, 4)};
        for (Shape s : shapes) {
            System.out.printf("%.2f %s%n", area(s), size(s));
        }
        System.out.println(firstBig(shapes));
        Expr e = new Add(new Num(2), new Mul(new Num(3), new Neg(new Neg(new Num(4)))));
        System.out.println(show(e) + " = " + eval(e));
        System.out.println(eval(new Mul(new Num(6), new Num(7))) + " " + show(new Num(-5)));
        Object[] things = {null, 5, 500, "", "hello", new Line(new Point(1, 2), new Point(1, 9)),
            new Line(new Point(0, 0), new Point(3, 4)), Color.BLUE, 2.5};
        for (Object o : things) {
            System.out.println(describe(o));
        }
        for (String text : new String[] {null, "y", "yes", "maybe", "absolutely"}) {
            System.out.println(classify(text));
        }
        for (Color c : Color.values()) {
            System.out.println(weight(c));
        }
        Shape missing = null;
        System.out.println(size(missing));
    }
}
