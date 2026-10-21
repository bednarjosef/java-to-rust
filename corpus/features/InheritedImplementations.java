// A class implements an interface's methods with methods it inherits from a superclass that does
// not implement the interface, as `Spliterators.EmptySpliterator.OfInt` does; one of them takes
// a type variable of the superclass, which the subclass's interface fixes. An enum implements an
// interface that declares `Object`'s `equals` again, as `Comparator` does.
public class InheritedImplementations {
    interface Sized {
        long size();

        int flags();
    }

    interface Consumer {
        void accept(int value);
    }

    interface Source extends Sized {
        boolean next(Consumer consumer);
    }

    abstract static class Empty<C> {
        public long size() {
            return 0;
        }

        public int flags() {
            return 64;
        }

        public boolean next(C consumer) {
            System.out.println("nothing for " + (consumer != null));
            return false;
        }
    }

    static final class EmptySource extends Empty<Consumer> implements Source {
    }

    static final class Counter implements Source {
        int left = 3;

        public long size() {
            return left;
        }

        public int flags() {
            return 1;
        }

        public boolean next(Consumer consumer) {
            if (left == 0) {
                return false;
            }
            consumer.accept(left--);
            return true;
        }
    }

    interface Order {
        int compare(int a, int b);

        boolean equals(Object other);
    }

    enum Natural implements Order {
        INSTANCE;

        public int compare(int a, int b) {
            return Integer.compare(a, b);
        }
    }

    static void drain(Source source) {
        System.out.println("size " + source.size() + " flags " + source.flags());
        while (source.next(v -> System.out.println("got " + v))) {
        }
    }

    public static void main(String[] args) {
        drain(new EmptySource());
        drain(new Counter());
        Order order = Natural.INSTANCE;
        Order other = Natural.INSTANCE;
        System.out.println(order.compare(1, 2) + " " + order.equals(other) + " " + order.equals("x"));
    }
}
