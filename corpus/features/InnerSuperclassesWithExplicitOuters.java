// Subclasses of inner classes given the superclass's enclosing instance explicitly: `o.new
// Inner() { .. }` for an anonymous class, in static and in instance code, and `o.super(..)` in a
// local class in static code, a static nested class and an inner class of an unrelated class.
// One subclass extends a class that already holds two enclosing instances, so it holds a third.
public class InnerSuperclassesWithExplicitOuters {
    static class Counter {
        final String name;
        int total;

        Counter(String name) { this.name = name; }

        class Step {
            final int by;

            Step(int by) { this.by = by; }

            int apply() {
                total += by;
                return total;
            }

            String describe() { return name + "+" + by; }
        }

        // A subclass in the same outer class whose superclass gets another Counter: `name` here
        // is this Counter's, and the Step's is the other one's.
        class Moved extends Step {
            Moved(Counter other, int by) { other.super(by); }

            @Override
            String describe() { return name + " moved " + super.describe(); }
        }

        class Batch {
            final int times;

            Batch(int times) { this.times = times; }

            class Repeated extends Step {
                Repeated(int by) { super(by * times); }

                @Override
                String describe() { return times + "x " + super.describe(); }
            }
        }
    }

    // An inner class whose code never uses its enclosing instance, which it does not hold.
    static class Labels {
        class Label {
            final String text;

            Label(String text) { this.text = text; }

            String show() { return "<" + text + ">"; }
        }

        String prefix = "#";

        class Tag {
            String tag() { return prefix + "tag"; }
        }

        // No constructor: javac's default one passes the enclosing instance on.
        class SubTag extends Tag {
            @Override
            String tag() { return "sub" + super.tag(); }
        }
    }

    // A static nested class extending an inner class: it has no enclosing instance of its own.
    static class Plain extends Counter.Step {
        Plain(Counter c, int by) { c.super(by); }

        @Override
        String describe() { return "plain " + super.describe(); }
    }

    static class Owner {
        final String owner;

        Owner(String owner) { this.owner = owner; }

        // An inner class of an unrelated class extending an inner class: it holds its own
        // enclosing instance as well as the superclass's.
        class Foreign extends Counter.Step {
            Foreign(Counter c, int by) { c.super(by); }

            @Override
            String describe() { return owner + " " + super.describe(); }
        }

        // The same, for a superclass that holds two: the Batch and, in its own superclass, the
        // Counter.
        class Again extends Counter.Batch.Repeated {
            Again(Counter.Batch b, int by) { b.super(by); }

            @Override
            String describe() { return owner + " again " + super.describe(); }
        }

        // An anonymous class with an explicit enclosing instance, in instance code.
        Counter.Step stepOf(Counter c, int by) {
            return c.new Step(by) {
                @Override
                String describe() { return owner + "'s " + super.describe(); }
            };
        }
    }

    static void run(Counter.Step step) {
        System.out.println(step.describe() + " -> " + step.apply());
    }

    public static void main(String[] args) {
        Counter c = new Counter("c");
        Counter d = new Counter("d");
        // An anonymous class with an explicit enclosing instance, in static code.
        run(c.new Step(2) {
            @Override
            String describe() { return "anonymous " + super.describe(); }
        });
        class Local extends Counter.Step {
            Local(Counter on, int by) { on.super(by); }

            @Override
            String describe() { return "local " + super.describe(); }
        }
        run(new Local(d, 3));
        run(new Plain(c, 4));
        Owner o = new Owner("o");
        run(o.new Foreign(d, 5));
        run(o.stepOf(c, 6));
        Counter.Batch b = d.new Batch(2);
        run(o.new Again(b, 7));
        Labels labels = new Labels();
        Labels.Label quoted = labels.new Label("q") {
            @Override
            String show() { return "\"" + text + "\""; }
        };
        System.out.println(quoted.show() + " " + labels.new Label("plain").show());
        System.out.println(labels.new SubTag().tag());
        run(c.new Moved(d, 1));
        System.out.println(c.total + " " + d.total);
    }
}
