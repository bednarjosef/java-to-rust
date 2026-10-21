// Sealed, final and non-sealed classes in an exhaustive switch statement, switch expressions
// yielding subclasses, block cases with yield, nested record patterns with var and total
// patterns, case null with continue, and instanceof chains.
import java.util.Objects;

public class SealedClassSwitch {
    sealed static abstract class Vehicle permits Car, Truck, Bike {
        abstract int wheels();
    }

    static final class Car extends Vehicle {
        final int seats;
        Car(int seats) { this.seats = seats; }
        int wheels() { return 4; }
    }

    static non-sealed class Truck extends Vehicle {
        final double load;
        Truck(double load) { this.load = load; }
        int wheels() { return 6; }
    }

    static final class Bike extends Vehicle {
        int wheels() { return 2; }
    }

    record Pair(Object first, Object second) {}

    record Box(Pair inner, String label) {}

    static int counter = 0;

    static Vehicle next() {
        counter++;
        return switch (counter % 3) {
            case 0 -> new Car(counter);
            case 1 -> new Truck(counter * 1.5);
            default -> new Bike();
        };
    }

    static String toll(Vehicle v) {
        String result;
        switch (v) {
            case Car c when c.seats > 5 -> result = "van";
            case Car c -> result = "car with " + c.seats;
            case Truck t -> {
                if (t.load > 3) {
                    result = "heavy truck";
                    break;
                }
                result = "light truck";
            }
            case Bike b -> result = "free";
        }
        return result + " (" + v.wheels() + " wheels)";
    }

    static String label(Object o) {
        return switch (o) {
            case Box(Pair(String a, Integer b), String l) when b > 1 -> {
                String text = a.repeat(b);
                yield l + ":" + text;
            }
            case Box(Pair(var a, var b), var l) -> {
                if (a == null) {
                    yield l + ": nothing";
                }
                yield l + ": " + a + "/" + b;
            }
            case Pair p -> "pair " + p.first();
            case Object other -> "object " + other;
        };
    }

    static int sum(Object[] values) {
        int total = 0;
        for (Object v : values) {
            switch (v) {
                case null:
                    continue;
                case Integer i:
                    total += i;
                    break;
                case String s:
                    total += s.length();
                    break;
                default:
                    total += 100;
            }
        }
        return total;
    }

    public static void main(String[] args) {
        for (int i = 0; i < 6; i++) {
            System.out.println(toll(next()));
        }
        System.out.println(toll(new Car(7)));
        System.out.println(label(new Box(new Pair("ab", 3), "x")));
        System.out.println(label(new Box(new Pair("ab", 1), "y")));
        System.out.println(label(new Box(new Pair(null, null), "z")));
        System.out.println(label(new Pair(1, 2)));
        System.out.println(label("hi"));
        System.out.println(sum(new Object[] {1, null, "abc", 2.0, 4}));
        Object o = new Pair("k", 9);
        if (o instanceof Pair p && p.second() instanceof Integer n) {
            System.out.println(p.first() + "=" + n);
        }
        Object same = new Pair("k", 9);
        System.out.println(Objects.equals(same, o) + " " + Objects.hash(1, 2));
    }
}
