// Statements before `this(..)` or `super(..)` (Java 25's flexible constructor bodies), as the
// JDK's `SortedOps` writes them: locals that the call's arguments and the rest of the body use,
// checks that throw before the object exists, effects that run before the superclass's
// constructor and the field initializers, and fields assigned early, which a superclass's
// constructor already sees through an overridden method.
public class FlexibleConstructors {
    static final StringBuilder log = new StringBuilder();

    static class Shape {
        final String name;
        final int sides;

        Shape(String name, int sides) {
            log.append("shape ").append(name).append("; ");
            this.name = name;
            this.sides = sides;
        }
    }

    static final class Polygon extends Shape {
        final int perimeter;
        int tag = next("tag");

        Polygon(int sides, int length) {
            if (sides < 3) {
                throw new IllegalArgumentException("too few sides: " + sides);
            }
            String name = sides == 3 ? "triangle" : sides + "-gon";
            log.append("checked ").append(name).append("; ");
            super(name, sides);
            this.perimeter = sides * length;
            log.append("perimeter ").append(perimeter).append(" for ").append(name).append("; ");
        }

        Polygon(int sides) {
            int length = sides * 2;
            this(sides, length);
        }
    }

    static int counter;

    static int next(String what) {
        log.append(what).append("; ");
        return ++counter;
    }

    // A superclass whose constructor calls a method that a subclass overrides.
    abstract static class Named {
        final String label;

        Named() {
            label = "<" + describe() + ">";
        }

        abstract String describe();
    }

    static final class Person extends Named {
        final String first;
        final String last;

        Person(String full) {
            int space = full.indexOf(' ');
            first = full.substring(0, space);
            this.last = full.substring(space + 1);
            super();
        }

        @Override
        String describe() {
            return last + ", " + first;
        }
    }

    // A class whose superclass is `Object`.
    static final class Range {
        final int low;
        final int high;

        Range(int a, int b) {
            int low = Math.min(a, b);
            int high = Math.max(a, b);
            super();
            this.low = low;
            this.high = high;
        }
    }

    public static void main(String[] args) {
        Polygon p = new Polygon(3, 5);
        System.out.println(p.name + " " + p.sides + " " + p.perimeter + " " + p.tag + " | " + log);
        log.setLength(0);
        Polygon q = new Polygon(6);
        System.out.println(q.name + " " + q.perimeter + " " + q.tag + " | " + log);
        log.setLength(0);
        try {
            new Polygon(2, 1);
        } catch (IllegalArgumentException e) {
            System.out.println("rejected: " + e.getMessage() + " | " + log);
        }
        Person ada = new Person("Ada Lovelace");
        System.out.println(ada.label + " " + ada.first + " " + ada.last);
        Range r = new Range(9, 4);
        System.out.println(r.low + ".." + r.high);
    }
}
