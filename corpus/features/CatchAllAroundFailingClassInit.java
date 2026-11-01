// A class whose static initialization fails, first used inside a catch of Throwable several
// calls down: the first use throws ExceptionInInitializerError, each later one
// NoClassDefFoundError, and a class used inside a catch-all that initializes fine runs once.
public class CatchAllAroundFailingClassInit {
    static class Broken {
        static int value = compute(4);

        static int compute(int n) {
            System.out.println("computing " + n);
            if (n > 3) {
                throw new IllegalStateException("no value for " + n);
            }
            return n;
        }
    }

    static class Fine {
        static int value = twice(2);

        static int twice(int n) {
            System.out.println("initializing fine");
            return n * 2;
        }
    }

    static int read() {
        return Broken.value + 1;
    }

    static int readFine() {
        return Fine.value + 1;
    }

    static int through() {
        return read() * 2;
    }

    public static void main(String[] args) {
        for (int i = 0; i < 3; i++) {
            try {
                System.out.println(through());
            } catch (Throwable t) {
                System.out.println(t.getClass().getName() + " " + t.getMessage());
                if (t.getCause() != null) {
                    System.out.println("cause " + t.getCause());
                }
            }
            try {
                System.out.println(readFine());
            } catch (Exception e) {
                System.out.println("unexpected " + e);
            }
        }
    }
}
