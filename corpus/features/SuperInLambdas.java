import java.util.function.Function;
import java.util.function.Supplier;

public class SuperInLambdas {
    interface Namer { String name(); }

    static class Fixed implements Namer {
        public String name() { return "fixed"; }
    }

    static class Base {
        String name() { return "base"; }
        int scale(int x) { return x * 2; }
        public String toString() { return "Base!"; }
    }

    static class Derived extends Base {
        String name() { return "derived"; }
        int scale(int x) { return x * 10; }

        void run() {
            Supplier<String> s = () -> super.name() + "/" + name();
            System.out.println(s.get());
            Function<Integer, Integer> f = super::scale;
            System.out.println(f.apply(7) + " " + scale(7));
            Runnable r = () -> System.out.println("inside: " + super.toString() + " " + super.scale(3));
            r.run();
            Namer[] namers = { new Fixed(), () -> super.name() + "!", super::name, this::name };
            for (Namer n : namers) {
                System.out.println(n.name());
            }
        }
    }

    static class Plain {
        int id = 5;
        void run() {
            Supplier<Boolean> same = () -> super.equals(this);
            Supplier<Integer> hash = () -> super.hashCode();
            System.out.println(same.get() + " " + (hash.get() == System.identityHashCode(this)) + " " + id);
        }
    }

    public static void main(String[] args) {
        new Derived().run();
        new Plain().run();
    }
}
