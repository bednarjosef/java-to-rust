public class StaticBlockThrows {
    static class Inner {
        static int size;

        static {
            System.out.println("inner block");
            if (size == 0) {
                throw new IllegalStateException("empty");
            }
        }
    }

    static class Outer {
        static int doubled = Inner.size * 2;

        static int run() {
            return doubled + 1;
        }
    }

    static int ask(int n) {
        System.out.println("asking " + n);
        return Outer.run() + n;
    }

    public static void main(String[] args) {
        System.out.println("start");
        System.out.println(ask(1));
        System.out.println("unreachable");
    }
}
