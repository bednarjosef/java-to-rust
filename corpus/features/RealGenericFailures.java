// Exceptions in a Rust generic: a generic stack (a struct with a type parameter in the
// translation) whose pop throws on an empty stack, caught around a loop with a finally.
public class RealGenericFailures {
    static class EmptyStackException extends RuntimeException {
        EmptyStackException(String message) {
            super(message);
        }
    }

    static class Stack<T> {
        private T top;
        private Stack<T> below;
        private int size;

        void push(T value) {
            Stack<T> rest = new Stack<>();
            rest.top = top;
            rest.below = below;
            rest.size = size;
            top = value;
            below = rest;
            size++;
        }

        T pop() {
            Stack<T> next = below;
            if (next == null) {
                throw new EmptyStackException("pop of an empty stack");
            } else {
                T value = top;
                top = next.top;
                size--;
                below = next.below;
                return value;
            }
        }
    }

    public static void main(String[] args) {
        Stack<String> stack = new Stack<>();
        stack.push("a");
        stack.push("b");
        try {
            for (int i = 0; i < 3; i++) {
                System.out.println(stack.pop());
            }
        } catch (EmptyStackException e) {
            System.out.println("caught " + e.getMessage());
        } finally {
            System.out.println("done");
        }
    }
}
