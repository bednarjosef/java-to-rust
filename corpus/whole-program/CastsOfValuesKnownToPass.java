// Casts and instanceof tests of values whose classes the points-to facts of the whole program
// know (Class::tested): the lists of an EnumMap, cast to List as they are read, as an entity's
// collector of block effects keeps them; a field of type Object that only ever holds shapes;
// fields of a class type that only hold shapes, or only labels, whose tests pass or fail for
// every object; and values that may be shapes, strings or labels. The translation tests fewer
// classes, or none, and must pass and fail the same objects as Java.
import java.util.ArrayList;
import java.util.EnumMap;
import java.util.EnumSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.function.Consumer;

public class CastsOfValuesKnownToPass {
    enum Kind {
        FREEZE,
        IGNITE,
        EXTINGUISH
    }

    interface Shape {
        int area();
    }

    abstract static class Thing {
    }

    static final class Square extends Thing implements Shape {
        final int s;

        Square(int s) {
            this.s = s;
        }

        public int area() {
            return s * s;
        }

        public String toString() {
            return "Square " + s;
        }
    }

    static final class Circle extends Thing implements Shape {
        public int area() {
            return 3;
        }

        public String toString() {
            return "Circle";
        }
    }

    static final class Label extends Thing {
        final String text;

        Label(String text) {
            this.text = text;
        }

        public String toString() {
            return "Label " + text;
        }
    }

    /// Effects kept by kind until a step ends, then run in the order of the kinds.
    static final class Collector {
        final Map<Kind, List<Consumer<StringBuilder>>> before = new EnumMap<>(Kind.class);
        final Set<Kind> pending = EnumSet.noneOf(Kind.class);
        final List<Consumer<StringBuilder>> done = new ArrayList<>();

        Collector() {
            for (Kind k : Kind.values()) {
                before.put(k, new ArrayList<>());
            }
        }

        void runBefore(Kind k, Consumer<StringBuilder> effect) {
            before.get(k).add(effect);
        }

        void flush() {
            for (Kind k : Kind.values()) {
                List<Consumer<StringBuilder>> effects = before.get(k);
                done.addAll(effects);
                effects.clear();
                if (pending.remove(k)) {
                    done.add(out -> out.append(k.name().toLowerCase()).append(' '));
                }
            }
        }
    }

    static Object held;
    static Thing shapeThing;
    static Thing labelThing;

    static Object mixed(int i) {
        switch (i % 4) {
            case 0:
                return new Square(i);
            case 1:
                return "text " + i;
            case 2:
                return new Circle();
            default:
                return new Label("l" + i);
        }
    }

    public static void main(String[] args) {
        Collector collector = new Collector();
        for (int step = 0; step < 3; step++) {
            int s = step;
            collector.runBefore(Kind.values()[step], out -> out.append("step ").append(s).append(' '));
            collector.pending.add(Kind.values()[(step + 1) % 3]);
            collector.flush();
        }
        StringBuilder out = new StringBuilder();
        for (Consumer<StringBuilder> effect : collector.done) {
            effect.accept(out);
        }
        System.out.println(out.toString().trim());

        // `held` only ever holds shapes: the cast cannot fail.
        int total = 0;
        held = new Square(3);
        for (int i = 0; i < 4; i++) {
            Shape s = (Shape) held;
            total += s.area();
            System.out.println(s);
            held = i % 2 == 0 ? new Circle() : new Square(i);
        }
        // `shapeThing` only ever holds shapes and `labelThing` only labels (or null): each test
        // passes or fails for every object it can hold, and the cast of a label fails.
        Thing[] things = { new Square(2), new Label("t"), new Circle() };
        for (int i = 0; i < 4; i++) {
            shapeThing = i % 2 == 0 ? new Circle() : new Square(i);
            labelThing = i == 3 ? null : new Label("k" + i);
            System.out.println((shapeThing instanceof Shape) + " " + (labelThing instanceof Shape));
            total += ((Shape) shapeThing).area();
            try {
                Shape s = (Shape) labelThing;
                System.out.println("cast " + s);
            } catch (ClassCastException e) {
                System.out.println("not a shape: " + labelThing + ": " + e.getMessage());
            }
            Thing t = things[i % things.length];
            if (t instanceof Shape s) {
                total += s.area();
            }
        }
        // A value that may be a shape, a string or a label.
        for (int i = 0; i < 8; i++) {
            Object o = mixed(i);
            if (o instanceof Shape) {
                total += ((Shape) o).area();
            } else if (o instanceof CharSequence cs) {
                total += cs.length();
            }
            try {
                Shape s = (Shape) o;
                System.out.println("shape " + s);
            } catch (ClassCastException e) {
                System.out.println("not a shape: " + o + ": " + e.getMessage());
            }
        }
        System.out.println(total);
    }
}
