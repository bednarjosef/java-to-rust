import java.util.concurrent.atomic.AtomicReferenceFieldUpdater;

// Field updaters made in a static block from classes that a generic helper casts
// (`cls(Node.class)`), held in the block's locals, one of a field whose type is a type variable,
// as netty's `ConcurrentSkipListIntObjMultimap` makes its updaters, which its
// `AdaptivePoolingAllocator` uses.
public class UpdatersOfClassesFromHelpers {
    static final class Chain<V> {
        static final class Node<V> {
            final int key;
            volatile V val;
            volatile Node<V> next;

            Node(int key, V val, Node<V> next) {
                this.key = key;
                this.val = val;
                this.next = next;
            }
        }

        static final class Index<V> {
            final Node<V> node;
            volatile Index<V> right;

            Index(Node<V> node, Index<V> right) {
                this.node = node;
                this.right = right;
            }
        }

        private volatile Index<V> head;
        private final Node<V> first = new Node<>(Integer.MIN_VALUE, null, null);

        private static final AtomicReferenceFieldUpdater<Chain<?>, Index<?>> HEAD;
        private static final AtomicReferenceFieldUpdater<Node<?>, Node<?>> NEXT;
        private static final AtomicReferenceFieldUpdater<Node<?>, Object> VAL;

        static {
            try {
                Class<Chain<?>> chainCls = cls(Chain.class);
                Class<Index<?>> indexCls = cls(Index.class);
                Class<Node<?>> nodeCls = cls(Node.class);
                HEAD = AtomicReferenceFieldUpdater.newUpdater(chainCls, indexCls, "head");
                NEXT = AtomicReferenceFieldUpdater.newUpdater(nodeCls, nodeCls, "next");
                VAL = AtomicReferenceFieldUpdater.newUpdater(nodeCls, Object.class, "val");
            } catch (IllegalArgumentException e) {
                throw new ExceptionInInitializerError(e);
            }
        }

        @SuppressWarnings("unchecked")
        private static <T> Class<T> cls(Class<?> cls) {
            return (Class<T>) cls;
        }

        void put(int key, V value) {
            Node<V> b = first;
            for (;;) {
                Node<V> n = b.next;
                if (n == null || n.key > key) {
                    if (NEXT.compareAndSet(b, n, new Node<V>(key, value, n))) {
                        break;
                    }
                } else if (n.key == key) {
                    VAL.set(n, value);
                    break;
                } else {
                    b = n;
                }
            }
            Index<V> h = head;
            HEAD.compareAndSet(this, h, new Index<V>(first.next, h));
        }

        static int made;

        static Node<String> counted(int key) {
            made++;
            return new Node<>(key, "counted", null);
        }

        // The updater tests its object once the other arguments are computed.
        static String refused() {
            Node<String> none = null;
            try {
                NEXT.compareAndSet(none, null, counted(1));
                return "set";
            } catch (ClassCastException e) {
                return "refused after " + made + " made, message " + e.getMessage();
            }
        }

        String text() {
            StringBuilder out = new StringBuilder();
            for (Node<V> n = first.next; n != null; n = n.next) {
                out.append(n.key).append('=').append(n.val).append(' ');
            }
            int levels = 0;
            for (Index<V> i = head; i != null; i = i.right) {
                levels++;
            }
            return out.append(levels).toString();
        }
    }

    public static void main(String[] args) {
        Chain<String> chain = new Chain<>();
        chain.put(5, "five");
        chain.put(2, "two");
        chain.put(9, "nine");
        chain.put(5, "FIVE");
        System.out.println(chain.text());
        System.out.println(Chain.refused());
    }
}
