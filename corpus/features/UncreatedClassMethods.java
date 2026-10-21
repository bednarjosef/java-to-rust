// The instance methods of a class that the program never creates objects of never run, and
// their bodies may call what was never emitted for it, such as its interface's default methods.
public class UncreatedClassMethods {
    interface Labeled {
        int size();
        default String label() {
            return "L" + size();
        }
    }
    static class Used implements Labeled {
        public int size() {
            return 2;
        }
        @Override
        public String label() {
            return "U/" + Labeled.super.label();
        }
    }
    static class Unused implements Labeled, Comparable<Unused> {
        int k = 1;
        public int size() {
            return k;
        }
        @Override
        public String label() {
            return "N/" + Labeled.super.label() + size();
        }
        int twice(int a0) {
            k += a0;
            return label().length() * 2;
        }
        public int compareTo(Unused o) {
            return k - o.k;
        }
    }
    static class UnusedChild extends Used {
        int extra(int a0) {
            return size() + a0 + label().length();
        }
    }
    static Unused nothing = null;

    public static void main(String[] args) {
        Labeled l = new Used();
        System.out.println(l.label() + " " + l.size());
        Object o = l;
        System.out.println(o instanceof Unused);
        System.out.println(o instanceof UnusedChild);
        System.out.println(nothing == null);
    }
}
