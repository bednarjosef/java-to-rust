public class MonitorsInOneFamily {
    static class Gate {
    }

    static final Gate gate = new Gate();

    static class Descriptor {
        final String name;

        Descriptor(String name) { this.name = name; }

        String name() { return name; }

        String type(int k) { return name + "/" + k; }
    }

    // A subclass whose synchronized method keeps a monitor. The superclass's objects are values of
    // the same enum, so they change in the same way, and the fields are read through it.
    static Descriptor merged(String first) {
        return new Descriptor(first) {
            @Override
            synchronized String type(int k) { return "merged " + name + " " + super.type(k) + name(); }
        };
    }

    public static void main(String[] args) {
        Descriptor d = merged("a");
        Descriptor plain = new Descriptor("p");
        synchronized (gate) {
            gate.notifyAll();
        }
        System.out.println(d.type(1));
        System.out.println(plain.type(2));
        System.out.println(d.name + " " + plain.name);
        System.out.println(d.name() + plain.name());
    }
}
