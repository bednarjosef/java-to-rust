// The classes of objects: equals methods that compare them, class literals and their names.
public class ClassComparisons {
    static class Point {
        final int x, y;

        Point(int x, int y) {
            this.x = x;
            this.y = y;
        }

        @Override
        public boolean equals(Object o) {
            if (this == o) return true;
            if (o == null || getClass() != o.getClass()) return false;
            Point p = (Point) o;
            return x == p.x && y == p.y;
        }

        @Override
        public int hashCode() {
            return 31 * x + y;
        }
    }

    static class ColoredPoint extends Point {
        final String color;

        ColoredPoint(int x, int y, String color) {
            super(x, y);
            this.color = color;
        }
    }

    // A class without subclasses.
    static final class Money {
        final long cents;

        Money(long cents) {
            this.cents = cents;
        }

        @Override
        public boolean equals(Object o) {
            if (o == null || o.getClass() != getClass()) return false;
            return cents == ((Money) o).cents;
        }

        @Override
        public int hashCode() {
            return Long.hashCode(cents);
        }
    }

    abstract static class Animal {
        abstract String sound();
    }

    static class Dog extends Animal {
        String sound() { return "woof"; }
    }

    static class Cat extends Animal {
        String sound() { return "meow"; }
    }

    static String kind(Animal a) {
        if (a.getClass() == Dog.class) return "a dog";
        if (a.getClass().equals(Cat.class)) return "a cat";
        return "something else";
    }

    static boolean sameKind(Animal a, Animal b) {
        return a.getClass().equals(b.getClass());
    }

    static int made = 0;

    static Animal make(boolean dog) {
        made++;
        return dog ? new Dog() : new Cat();
    }

    static Point point() {
        made++;
        return new Point(0, 0);
    }

    public static void main(String[] args) {
        // The object is made once, whatever its class.
        boolean d = make(true).getClass() == Dog.class;
        boolean e = make(false).getClass() != make(false).getClass();
        boolean f = point().getClass() == Point.class;
        System.out.println(d + " " + e + " " + f + " " + made);
        Point p = new Point(1, 2);
        Point q = new Point(1, 2);
        Point c = new ColoredPoint(1, 2, "red");
        System.out.println(p.equals(q) + " " + p.equals(c) + " " + c.equals(p) + " " + p.equals(null));
        System.out.println(p.getClass() == q.getClass());
        System.out.println(p.getClass() != c.getClass());
        Animal[] animals = { new Dog(), new Cat(), new Dog() };
        for (Animal a : animals) {
            System.out.println(kind(a) + " says " + a.sound() + ", " + a.getClass().getSimpleName());
        }
        System.out.println(sameKind(animals[0], animals[2]) + " " + sameKind(animals[0], animals[1]));
        System.out.println(Dog.class.getName() + " " + Cat.class.getSimpleName() + " " + Point.class.getName());
        System.out.println(c.getClass() == ColoredPoint.class);
        Money m = new Money(250);
        Object other = new Money(250);
        System.out.println(m.equals(other) + " " + m.equals(p) + " " + m.equals(new Money(1)));
        System.out.println(m.getClass() == Money.class && !m.getClass().equals(Point.class));
    }
}
