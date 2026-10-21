// Exceptions through generics: a generic interface's method that throws a checked exception,
// implemented with a bridge (`Integer next()` for `T next()`) and caught in a generic method and
// through the interface; and a generic class whose constructor throws.
public class GenericFailures {
    static class EmptyException extends Exception {
        EmptyException(String message) {
            super(message);
        }
    }

    interface Source<T> {
        T next() throws EmptyException;
    }

    static class Counter implements Source<Integer> {
        private int left;

        Counter(int left) {
            this.left = left;
        }

        public Integer next() throws EmptyException {
            if (left == 0) {
                throw new EmptyException("counter empty");
            }
            left--;
            return left;
        }
    }

    static class Box<T> {
        private final T value;

        Box(T value) {
            if (value == null) {
                throw new IllegalArgumentException("no value");
            }
            this.value = value;
        }

        T get() {
            return value;
        }
    }

    static <T> int drain(Source<T> source) {
        int count = 0;
        try {
            while (true) {
                T item = source.next();
                count++;
                System.out.println("item " + item);
            }
        } catch (EmptyException e) {
            System.out.println("stopped: " + e.getMessage());
        }
        return count;
    }

    public static void main(String[] args) {
        System.out.println("drained " + drain(new Counter(3)));
        Box<String> box = new Box<>("hello");
        System.out.println(box.get().length());
        try {
            Box<String> empty = new Box<>(null);
            System.out.println(empty.get());
        } catch (IllegalArgumentException e) {
            System.out.println("caught " + e.getMessage());
        }
        Source<Integer> source = new Counter(1);
        try {
            int a = source.next();
            int b = source.next();
            System.out.println(a + b);
        } catch (EmptyException e) {
            System.out.println("second: " + e.getMessage());
        }
    }
}
