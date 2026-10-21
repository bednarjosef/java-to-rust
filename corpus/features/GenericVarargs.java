// Varargs of a type variable (`T... items`): the arguments are packed into an array of the
// variable's erasure, and an `Object[]` passed for it is passed on.
public class GenericVarargs {
    static class Stack<T> {
        private Object[] items = new Object[8];
        private int size;

        void push(T t) { items[size++] = t; }

        @SuppressWarnings("unchecked")
        T pop() { return (T) items[--size]; }

        boolean isEmpty() { return size == 0; }
    }

    @SafeVarargs
    static <T> void pushAll(Stack<T> stack, T... items) {
        for (T item : items) {
            stack.push(item);
        }
    }

    @SafeVarargs
    static <T> T last(T first, T... rest) {
        return rest.length == 0 ? first : rest[rest.length - 1];
    }

    static <T extends Comparable<T>> int countAbove(T limit, T... values) {
        int n = 0;
        for (T v : values) {
            if (v.compareTo(limit) > 0) {
                n++;
            }
        }
        return n;
    }

    static class Bag<E> {
        private final Object[] items;

        @SafeVarargs
        Bag(E... items) {
            this.items = items;
        }

        int size() { return items.length; }
    }

    public static void main(String[] args) {
        Stack<String> words = new Stack<>();
        pushAll(words, "a", "b", "c");
        pushAll(words);
        StringBuilder sb = new StringBuilder();
        while (!words.isEmpty()) {
            sb.append(words.pop());
        }
        System.out.println(sb);
        Stack<Integer> numbers = new Stack<>();
        pushAll(numbers, 1, 2, 3, 4);
        System.out.println(numbers.pop() + numbers.pop());
        System.out.println(last("only"));
        System.out.println(last(1, 2, 3));
        System.out.println(countAbove(10, 4, 11, 25, 10));
        System.out.println(countAbove("m", "a", "z", "q"));
        Object[] given = {"x", "y"};
        System.out.println(new Bag<>(given).size() + " " + new Bag<>("p", "q", "r").size() + " " + new Bag<String>().size());
    }
}
