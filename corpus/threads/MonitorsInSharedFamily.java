// A class tree that a thread reaches, where one subclass has a synchronized method (a kept
// monitor) and the others have none. The tree is changed in one way: each object of it is read
// through the same enum, so its objects are all locked, or none is.
public class MonitorsInSharedFamily {
    static class Descriptor {
        final String name;

        Descriptor(String name) { this.name = name; }

        String name() { return name; }

        String type(int k) { return name + "/" + k; }
    }

    static Descriptor merged(String first) {
        return new Descriptor(first) {
            @Override
            synchronized String type(int k) { return "merged " + name + " " + super.type(k) + name(); }
        };
    }

    static Descriptor shared;
    static Descriptor plain;

    static class Reader extends Thread {
        @Override
        public void run() {
            System.out.println(shared.type(1));
            System.out.println(plain.type(2));
            System.out.println(shared.name() + plain.name());
        }
    }

    public static void main(String[] args) throws InterruptedException {
        shared = merged("a");
        plain = new Descriptor("p");
        Reader reader = new Reader();
        reader.start();
        reader.join();
        System.out.println(shared.type(3) + " " + plain.name);
    }
}
