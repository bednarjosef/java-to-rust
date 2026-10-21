public class InnerIterators {
    interface Cursor {
        boolean hasNext();
        String next();
        void remove();
    }

    static class Names {
        private String[] items = new String[2];
        private int size;
        private int modifications;

        void add(String s) {
            if (size == items.length) {
                String[] bigger = new String[items.length * 2];
                for (int i = 0; i < size; i++) bigger[i] = items[i];
                items = bigger;
            }
            items[size++] = s;
            modifications++;
        }

        int size() { return size; }

        Cursor cursor() { return new Walker(); }

        // An inner iterator over the outer list, which supports `remove`.
        class Walker implements Cursor {
            private int next;
            private int last = -1;
            private final int expected = modifications;

            public boolean hasNext() { return next < size; }

            public String next() {
                if (expected != modifications) throw new IllegalStateException("changed");
                last = next;
                return items[next++];
            }

            public void remove() {
                if (last < 0) throw new IllegalStateException("nothing to remove");
                for (int i = last; i < size - 1; i++) items[i] = items[i + 1];
                items[--size] = null;
                next = last;
                last = -1;
            }
        }
    }

    public static void main(String[] args) {
        Names names = new Names();
        for (String s : new String[] { "ada", "bo", "cy", "dee", "ed" }) names.add(s);
        Cursor c = names.cursor();
        StringBuilder all = new StringBuilder();
        while (c.hasNext()) all.append(c.next()).append(' ');
        System.out.println(all.toString().trim());

        // Two cursors over one list at once.
        Cursor a = names.cursor();
        Cursor b = names.cursor();
        a.next();
        System.out.println(a.next() + " " + b.next() + " " + a.next());

        // Removing the names of two letters.
        Cursor r = names.cursor();
        while (r.hasNext()) {
            if (r.next().length() == 2) r.remove();
        }
        Cursor left = names.cursor();
        StringBuilder rest = new StringBuilder();
        while (left.hasNext()) rest.append(left.next()).append(',');
        System.out.println(rest + " size " + names.size());

        try {
            Cursor stale = names.cursor();
            names.add("fay");
            stale.next();
        } catch (IllegalStateException e) {
            System.out.println("stale cursor: " + e.getMessage());
        }
        try {
            names.cursor().remove();
        } catch (IllegalStateException e) {
            System.out.println("fresh cursor: " + e.getMessage());
        }
    }
}
