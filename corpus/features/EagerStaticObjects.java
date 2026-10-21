// Static final objects of classes whose initialization runs eagerly (it prints, or creates
// objects), as java.util.Collections keeps its empty iterator: once the class is initialized
// they are never null, so a catch of NullPointerException around their use sees nothing.
public class EagerStaticObjects {
    interface Source {
        boolean hasNext();

        int next();
    }

    static class Empty implements Source {
        static final Empty INSTANCE = new Empty();

        public boolean hasNext() {
            return false;
        }

        public int next() {
            throw new IllegalStateException("empty");
        }
    }

    static class Counter implements Source {
        static final Counter SHARED;

        static {
            System.out.println("initializing Counter");
            SHARED = new Counter(3);
        }

        int left;

        Counter(int left) {
            this.left = left;
        }

        public boolean hasNext() {
            return left > 0;
        }

        public int next() {
            return left--;
        }
    }

    static Source source(int kind) {
        return kind == 0 ? Empty.INSTANCE : Counter.SHARED;
    }

    static int sum(int kind) {
        try {
            int total = 0;
            Source s = source(kind);
            while (s.hasNext()) {
                total += s.next();
            }
            return Empty.INSTANCE.hasNext() ? -2 : total;
        } catch (NullPointerException e) {
            return -1;
        }
    }

    public static void main(String[] args) {
        System.out.println("start");
        System.out.println(sum(0));
        System.out.println(sum(1));
        System.out.println(sum(1));
        System.out.println(Empty.INSTANCE == source(0));
    }
}
