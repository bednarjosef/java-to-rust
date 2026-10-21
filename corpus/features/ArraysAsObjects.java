// Arrays held as `Object`s: in a generic container (`Queue<int[]>`), in an `Object` variable and
// cast back, tested with `instanceof`, compared by identity, and shared, so a change through one
// reference shows in the other.
public class ArraysAsObjects {
    static class Queue<T> {
        private Object[] items = new Object[4];
        private int head;
        private int tail;

        void offer(T t) {
            if (tail == items.length) {
                Object[] bigger = new Object[items.length * 2];
                System.arraycopy(items, 0, bigger, 0, items.length);
                items = bigger;
            }
            items[tail++] = t;
        }

        @SuppressWarnings("unchecked")
        T poll() { return head == tail ? null : (T) items[head++]; }

        boolean isEmpty() { return head == tail; }
    }

    static class Pair<A, B> {
        final A first;
        final B second;

        Pair(A first, B second) {
            this.first = first;
            this.second = second;
        }
    }

    static int sum(Object o) {
        int[] values = (int[]) o;
        int total = 0;
        for (int v : values) {
            total += v;
        }
        return total;
    }

    public static void main(String[] args) {
        Queue<int[]> cells = new Queue<>();
        for (int i = 0; i < 6; i++) {
            cells.offer(new int[] {i, i * i});
        }
        StringBuilder sb = new StringBuilder();
        while (!cells.isEmpty()) {
            int[] cell = cells.poll();
            sb.append(cell[0]).append(':').append(cell[1]).append(' ');
        }
        System.out.println(sb.toString().trim());
        System.out.println(cells.poll() == null);

        Queue<String[]> rows = new Queue<>();
        rows.offer(new String[] {"a", "b"});
        rows.offer(new String[] {"c"});
        System.out.println(String.join(",", rows.poll()) + " " + rows.poll().length);

        int[] shared = {1, 2, 3};
        Object held = shared;
        shared[0] = 10;
        System.out.println(sum(held));
        int[] back = (int[]) held;
        back[2] = 30;
        System.out.println(shared[2] + " " + (back == shared) + " " + (held == shared));
        Object other = new int[] {10, 2, 30};
        System.out.println(held == other);
        System.out.println(held.equals(shared) + " " + held.equals(other));
        System.out.println(held.hashCode() == shared.hashCode());

        Object[] things = {shared, new String[] {"s"}, new double[2]};
        for (Object thing : things) {
            if (thing instanceof int[] ints) {
                System.out.println("ints " + ints.length);
            } else if (thing instanceof String[]) {
                System.out.println("strings " + ((String[]) thing)[0]);
            } else {
                System.out.println("other " + (thing == shared));
            }
        }

        Pair<int[], double[][]> pair = new Pair<>(new int[] {7}, new double[][] {{0.5, 1.5}});
        System.out.println(pair.first[0] + pair.second[0][1]);
    }
}
