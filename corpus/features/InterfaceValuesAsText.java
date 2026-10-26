import java.util.function.Supplier;

// Values whose static type is an interface or `Object` turned into text, by concatenation,
// `String.valueOf`, `toString()` and printing, whatever classes implement the interface: only an
// enum, only a record, only lambdas, a class with its own `toString`, and the runtime's classes.
public class InterfaceValuesAsText {
    interface Reader {
        int read();
    }

    enum Policy implements Reader {
        WHOLE,
        HALF {
            @Override
            public String toString() {
                return "half!";
            }
        };

        public int read() {
            return ordinal() + 1;
        }

        @Override
        public String toString() {
            return name().toLowerCase();
        }
    }

    // An enum that keeps `Enum`'s `toString`.
    enum Plain implements Reader {
        ONE;

        public int read() {
            return 1;
        }
    }

    interface Shape {
        double area();
    }

    record Square(double side) implements Shape {
        public double area() {
            return side * side;
        }
    }

    interface Named {
        String name();
    }

    static class Person implements Named {
        public String name() {
            return "ada";
        }

        @Override
        public String toString() {
            return "Person(" + name() + ")";
        }
    }

    // The constants of `Policy`, last first.
    static class Backwards implements Iterable<Policy> {
        public java.util.Iterator<Policy> iterator() {
            return new java.util.Iterator<Policy>() {
                int at = Policy.values().length;

                public boolean hasNext() {
                    return at > 0;
                }

                public Policy next() {
                    return Policy.values()[--at];
                }
            };
        }
    }

    static Reader first() {
        return Policy.values()[0];
    }

    static void show(Object o) {
        System.out.println("object: " + o + " " + String.valueOf(o) + " " + o.toString());
    }

    public static void main(String[] args) {
        for (Reader r : Policy.values()) {
            System.out.println("reader " + r + " " + r.read());
            System.out.println(String.valueOf(r) + " " + r.toString());
            System.out.println(r);
            show(r);
        }
        for (Reader r : new Backwards()) {
            System.out.println("listed " + r);
        }
        Reader chosen = args.length > 5 ? Policy.WHOLE : Policy.HALF;
        System.out.println("chosen " + chosen + " " + first());
        Reader[] readers = Policy.values();
        System.out.println("array " + readers[1] + " " + readers.length);
        Reader plain = Plain.ONE;
        System.out.println("plain " + plain + " " + plain.toString() + " " + String.valueOf(plain));
        Shape s = new Square(2);
        System.out.println("shape " + s + " " + s.area());
        System.out.println(s.toString() + " " + String.valueOf(s));
        System.out.println(s);
        show(s);

        Named n = new Person();
        System.out.println("named " + n + " " + n.toString() + " " + String.valueOf(n));
        show(n);

        Supplier<String> lambda = () -> "made";
        String lambdaText = "" + lambda;
        System.out.println(lambdaText.contains("Lambda") || lambdaText.contains("$$") ? "a lambda" : lambdaText);
        System.out.println(lambda.get());

        CharSequence text = args.length > 5 ? new StringBuilder("builder") : "plain";
        System.out.println("text " + text + " " + text.toString() + " " + String.valueOf(text));
        System.out.println(text);
        show(text);

        Comparable<Integer> number = 42;
        System.out.println("number " + number + " " + number.toString());
        show(number);

        Object[] values = {Policy.HALF, new Square(1), new Person(), "s", 7, 'c', 2.5, true, null};
        for (Object v : values) {
            System.out.println("value " + v + " " + String.valueOf(v));
        }
    }
}
