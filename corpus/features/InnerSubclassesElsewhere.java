// Classes extending an inner class from outside its outer class's own members: a local and an
// anonymous class, an inner class of an inner class, and an inner class of a subclass of the
// outer class. Each passes the superclass's enclosing instance to its constructor.
public class InnerSubclassesElsewhere {
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

        // A local class extending the inner class, in an instance method.
        Step doubled(int amount) {
            class Doubled extends Step {
                Doubled(int twice) { super(twice); }

                @Override
                String describe() { return "double " + amount + " " + by + " " + super.describe(); }
            }
            return new Doubled(amount * 2);
        }

        // An anonymous class extending the inner class, with a constructor argument.
        Step loud(int by) {
            return new Step(by) {
                @Override
                String describe() { return super.describe().toUpperCase() + "!"; }
            };
        }

        // An inner class of an inner class extending the outermost inner class: its own outer is
        // a Batch, and the Step's is the Counter that the Batch belongs to.
        class Batch {
            final int times;

            Batch(int times) { this.times = times; }

            class Repeated extends Step {
                Repeated(int by) { super(by * times); }

                @Override
                String describe() { return times + "x " + super.describe(); }
            }

            Step make(int by) { return new Repeated(by); }

            // A local class in an inner class's method, extending the outer's inner class.
            Step offset(int amount) {
                class Offset extends Step {
                    Offset(int start) { super(start + times); }

                    @Override
                    String describe() { return "offset " + amount + " " + by + " " + super.describe(); }
                }
                return new Offset(amount);
            }

            // An anonymous class there, too.
            Step quiet(int by) {
                return new Step(by) {
                    @Override
                    String describe() { return "quiet " + times; }
                };
            }
        }
    }

    // A subclass of the outer class whose inner class extends the superclass's inner class.
    static class NamedCounter extends Counter {
        final String prefix;

        NamedCounter(String name, String prefix) {
            super(name);
            this.prefix = prefix;
        }

        class Tagged extends Step {
            Tagged(int by) { super(by); }

            @Override
            String describe() { return prefix + ":" + super.describe(); }
        }

        Step tagged(int by) { return new Tagged(by); }
    }

    static void run(Counter.Step step) {
        System.out.println(step.describe() + " -> " + step.apply());
    }

    public static void main(String[] args) {
        Counter c = new Counter("c");
        run(c.new Step(1));
        run(c.doubled(3));
        run(c.loud(5));
        Counter.Batch batch = c.new Batch(3);
        run(batch.make(2));
        run(batch.offset(1));
        run(batch.quiet(10));
        Counter.Batch.Repeated r = batch.new Repeated(1);
        run(r);
        NamedCounter n = new NamedCounter("n", "tag");
        run(n.tagged(4));
        run(n.new Tagged(6));
        run(n.doubled(1));
        System.out.println(c.total + " " + n.total);
    }
}
