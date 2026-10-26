// A method of a generic superclass that implements an interface method only in a subclass,
// which fixes the superclass's type arguments: the subclass bridges to it, as Nodes.EmptyNode.OfRef
// does to copyInto and forEach; another subclass keeps the interface's default.
import java.util.function.Consumer;
import java.util.function.IntConsumer;
import java.util.function.IntFunction;
public class InheritedGenericBridge {
    interface Node<T> {
        default void forEach(Consumer<? super T> consumer) { System.out.println("node default"); }
        void copyInto(T[] array, int offset);
        T[] asArray(IntFunction<T[]> generator);
        long count();
    }
    interface NodeOfInt extends Node<Integer> {
        default void forEach(IntConsumer consumer) { System.out.println("int default"); }
        default void copyInto(Integer[] array, int offset) { }
        void copyInto(int[] array, int offset);
    }
    abstract static class EmptyNode<T, A, C> implements Node<T> {
        public T[] asArray(IntFunction<T[]> generator) { return generator.apply(0); }
        public void copyInto(A array, int offset) { System.out.println("copy"); }
        public long count() { return 0; }
        public void forEach(C consumer) { System.out.println("empty"); }
        private static class OfRef<T> extends EmptyNode<T, T[], Consumer<? super T>> {
            private OfRef() { }
            @Override public String toString() { return "ofref"; }
        }
        private static final class OfInt extends EmptyNode<Integer, int[], IntConsumer> implements NodeOfInt {
            OfInt() { }
        }
    }
    public static void main(String[] args) {
        Node<String> a = new EmptyNode.OfRef<>();
        a.forEach(s -> {});
        a.copyInto(new String[0], 0);
        Node<Integer> b = new EmptyNode.OfInt();
        b.forEach((Consumer<Integer>) s -> {});
        ((NodeOfInt) b).forEach((IntConsumer) i -> {});
        ((NodeOfInt) b).copyInto(new int[0], 0);
        System.out.println(a.count() + b.count() + " " + a.asArray(String[]::new).length);
    }
}
