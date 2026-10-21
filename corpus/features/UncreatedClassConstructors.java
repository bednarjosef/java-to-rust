// Base values are all Made objects, so Base methods are emitted on Made; the constructors of
// Unmade, which no code calls, call them too, and get unreachable bodies like its methods.
public class UncreatedClassConstructors {
    static String acc = "";
    static class Base {
        int k = 1;
        int name() {
            return 1;
        }
        int twice(int a0) {
            return a0 * 2 + k + name();
        }
    }
    static class Made extends Base {
        Made() {
            acc += twice(3);
        }
        @Override
        int name() {
            return 7;
        }
    }
    static class Unmade extends Base {
        Unmade() {
            acc += twice(4);
            System.out.println("new Unmade " + k);
        }
        Unmade(int k) {
            this();
            this.k = k;
        }
    }

    public static void main(String[] args) {
        Base b = new Made();
        System.out.println(b.twice(5) + " " + acc);
        Object o = b;
        System.out.println(o instanceof Unmade);
    }
}
