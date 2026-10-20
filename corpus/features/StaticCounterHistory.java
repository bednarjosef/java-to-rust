public class StaticCounters {
    static int nextId = 1;
    static int[] history = new int[5];
    static int historyLen;
    static String lastName = "none";
    static long total;
    static final int CAPACITY = 3;

    static class Widget {
        final int id;
        final String name;
        Widget(String name) {
            this.name = name;
            this.id = nextId++;
            record(id);
            lastName = name;
        }
        static void record(int v) {
            if (historyLen < CAPACITY + 2) history[historyLen++] = v;
            total += v;
        }
        public String toString() { return "#" + id + ":" + name; }
    }

    static void reset() {
        nextId = 100;
        historyLen = 0;
        lastName = "reset";
    }

    static int peek() {
        return historyLen == 0 ? -1 : history[historyLen - 1];
    }

    public static void main(String[] args) {
        Widget a = new Widget("a");
        Widget b = new Widget("b");
        System.out.println(a + " " + b + " next=" + nextId);
        System.out.println(lastName + " " + total + " " + peek());
        Widget.record(50);
        System.out.println(peek() + " " + historyLen + " " + total);
        reset();
        Widget c = new Widget("c");
        System.out.println(c + " " + lastName + " " + historyLen);
        int[] h = history;
        h[0] = -7;
        System.out.println(history[0] + " " + h.length);
        StaticCounters.nextId += 5;
        System.out.println(nextId);
        nextId = nextId * 2;
        System.out.println(StaticCounters.nextId + " " + CAPACITY);
        long big = total * 1000000000L;
        System.out.println(big);
    }
}
