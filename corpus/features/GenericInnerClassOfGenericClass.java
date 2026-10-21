// A generic inner class of a generic class, whose type javac writes with both classes' type
// arguments (`Ring<T>.Walker<U>`), created where the interface it implements is expected; and an
// inner class of it named through them (`Ring<T>.Walker<U>.Step`).
public class GenericInnerClassOfGenericClass {
    interface Cursor<E> {
        boolean more();

        E take();
    }

    static class Ring<T> {
        final Object[] items;
        int start;

        Ring(Object[] items, int start) {
            this.items = items;
            this.start = start;
        }

        Cursor<T> cursor() {
            return new Walker<>();
        }

        private class Walker<U> implements Cursor<U> {
            int taken;
            Step last = new Step(-1);

            public boolean more() {
                return taken < items.length;
            }

            @SuppressWarnings("unchecked")
            public U take() {
                last = new Step((start + taken++) % items.length);
                return (U) items[last.at];
            }

            class Step {
                final int at;

                Step(int at) {
                    this.at = at;
                }
            }
        }
    }

    public static void main(String[] args) {
        Ring<String> ring = new Ring<>(new Object[] {"a", "b", "c", "d"}, 2);
        Cursor<String> cursor = ring.cursor();
        StringBuilder sb = new StringBuilder();
        while (cursor.more()) {
            sb.append(cursor.take());
        }
        System.out.println(sb);
        Ring<Integer> numbers = new Ring<>(new Object[] {1, 2, 3}, 1);
        int sum = 0;
        for (Cursor<Integer> c = numbers.cursor(); c.more(); ) {
            sum = sum * 10 + c.take();
        }
        System.out.println(sum);
    }
}
