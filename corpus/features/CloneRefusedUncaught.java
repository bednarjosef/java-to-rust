// `super.clone()` in a class that is not `Cloneable` compiles, and throws
// `CloneNotSupportedException` when it runs; here nothing catches it, so the program ends with
// exit code 1 after what it printed.
public class CloneRefusedUncaught {
    static class Counter {
        int count;

        Counter copy() throws CloneNotSupportedException {
            return (Counter) super.clone();
        }
    }

    // A Cloneable subclass of it copies fine.
    static class Tally extends Counter implements Cloneable {
        String label = "tally";
    }

    public static void main(String[] args) throws CloneNotSupportedException {
        Tally t = new Tally();
        t.count = 4;
        Counter copied = t.copy();
        copied.count++;
        System.out.println(t.count + " " + copied.count + " " + ((Tally) copied).label);
        Counter c = new Counter();
        System.out.println("copying a counter");
        c.copy();
        System.out.println("not reached");
    }
}
