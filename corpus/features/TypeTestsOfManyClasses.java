import java.util.function.IntSupplier;

// Type tests of values of many classes held as Objects: `instanceof` an interface whose classes
// are many runs of class numbers, and `Class.isInstance` and `Class.cast` of classes with
// subclasses, of leaves and of interfaces, against the program's objects, enum constants with
// bodies, records, lambdas, strings, boxes, arrays and null.
public class TypeTestsOfManyClasses {
    interface Shape { int sides(); }

    abstract static class Entity { }

    abstract static class B00 extends Entity { }
    static final class S00 extends B00 implements Shape {
        public int sides() { return 3; }
    }
    static final class P00 extends B00 { }
    abstract static class B01 extends Entity { }
    static final class S01 extends B01 implements Shape {
        public int sides() { return 4; }
    }
    static final class P01 extends B01 { }
    abstract static class B02 extends Entity { }
    static final class S02 extends B02 implements Shape {
        public int sides() { return 5; }
    }
    static final class P02 extends B02 { }
    abstract static class B03 extends Entity { }
    static final class S03 extends B03 implements Shape {
        public int sides() { return 6; }
    }
    static final class P03 extends B03 { }
    abstract static class B04 extends Entity { }
    static final class S04 extends B04 implements Shape {
        public int sides() { return 7; }
    }
    static final class P04 extends B04 { }
    abstract static class B05 extends Entity { }
    static final class S05 extends B05 implements Shape {
        public int sides() { return 8; }
    }
    static final class P05 extends B05 { }
    abstract static class B06 extends Entity { }
    static final class S06 extends B06 implements Shape {
        public int sides() { return 3; }
    }
    static final class P06 extends B06 { }
    abstract static class B07 extends Entity { }
    static final class S07 extends B07 implements Shape {
        public int sides() { return 4; }
    }
    static final class P07 extends B07 { }
    abstract static class B08 extends Entity { }
    static final class S08 extends B08 implements Shape {
        public int sides() { return 5; }
    }
    static final class P08 extends B08 { }
    abstract static class B09 extends Entity { }
    static final class S09 extends B09 implements Shape {
        public int sides() { return 6; }
    }
    static final class P09 extends B09 { }
    abstract static class B10 extends Entity { }
    static final class S10 extends B10 implements Shape {
        public int sides() { return 7; }
    }
    static final class P10 extends B10 { }
    abstract static class B11 extends Entity { }
    static final class S11 extends B11 implements Shape {
        public int sides() { return 8; }
    }
    static final class P11 extends B11 { }

    static class Mob extends Entity implements Shape {
        public int sides() { return 1; }
    }

    static class Zombie extends Mob { }

    enum Op implements Shape {
        ADD { public int sides() { return 2; } },
        NEG;
        public int sides() { return 0; }
    }

    record Point(int x, int y) implements Shape {
        public int sides() { return x + y; }
    }

    static String tests(Object o, Class<?>[] classes) {
        StringBuilder b = new StringBuilder();
        for (Class<?> c : classes) {
            b.append(c.isInstance(o) ? '1' : '0');
            try {
                c.cast(o);
                b.append('+');
            } catch (ClassCastException e) {
                b.append('-');
            }
        }
        if (o instanceof Shape s) {
            b.append(" shape ").append(s.sides());
        }
        return b.toString();
    }

    public static void main(String[] args) {
        IntSupplier lambda = () -> 7;
        Object[] values = { new S00(), new P00(), new S01(), new P01(), new S02(), new P02(), new S03(), new P03(), new S04(), new P04(), new S05(), new P05(), new S06(), new P06(), new S07(), new P07(), new S08(), new P08(), new S09(), new P09(), new S10(), new P10(), new S11(), new P11(), new Mob(), new Zombie(), Op.ADD, Op.NEG, new Point(1, 2),
            lambda, "text", 42, new int[] { 1 }, new String[] { "a" }, new Object(), null };
        Class<?>[] classes = { Entity.class, B03.class, S05.class, P07.class, Mob.class, Zombie.class,
            Shape.class, Op.class, Point.class, Record.class, Enum.class, IntSupplier.class,
            Comparable.class, CharSequence.class, Number.class, Object.class, Runnable.class };
        for (int i = 0; i < values.length; i++) {
            System.out.println(i + " " + tests(values[i], classes));
        }
        int shapes = 0;
        for (int r = 0; r < 100; r++) {
            for (Object o : values) {
                if (o instanceof Shape) {
                    shapes++;
                }
                if (Entity.class.isInstance(o) && !Shape.class.isInstance(o)) {
                    shapes += 1000;
                }
            }
        }
        System.out.println(shapes);
    }
}
