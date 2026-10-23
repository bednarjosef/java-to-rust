import java.util.ArrayList;
import java.util.List;
import java.util.function.Consumer;
import java.util.function.Supplier;

// A bound method reference checks its receiver for null where it is made, before anything calls
// it, with an exception that has no message.
public class BoundReferenceReceivers {
    final List<String> seen;
    List<String> later = null;

    BoundReferenceReceivers(List<String> seen) {
        this.seen = seen;
    }

    static String show(Object p) {
        Supplier<String> s = p::toString;
        return s.get();
    }

    static void make(Object p) {
        Supplier<String> s = p::toString;
        System.out.println("made");
    }

    Consumer<String> adder() {
        return seen::add;
    }

    Consumer<String> laterAdder() {
        return later::add;
    }

    public static void main(String[] args) {
        System.out.println(show("x"));
        try {
            System.out.println(show(null));
        } catch (NullPointerException e) {
            System.out.println("show " + e.getMessage());
        }
        try {
            make(null);
        } catch (NullPointerException e) {
            System.out.println("make " + e.getMessage());
        }
        BoundReferenceReceivers full = new BoundReferenceReceivers(new ArrayList<>());
        full.adder().accept("a");
        System.out.println(full.seen);
        try {
            new BoundReferenceReceivers(null).adder();
            System.out.println("made an adder");
        } catch (NullPointerException e) {
            System.out.println("adder " + e.getMessage());
        }
        try {
            full.laterAdder();
            System.out.println("made a later adder");
        } catch (NullPointerException e) {
            System.out.println("later " + e.getMessage());
        }
        List.of("b", "c").forEach(System.out::println);
    }
}
