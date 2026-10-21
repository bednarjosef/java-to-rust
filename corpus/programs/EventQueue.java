// An event queue whose handlers are anonymous classes of a program interface, registered in a
// growing array and run in order for each event. One handler unregisters itself after its
// second event; another counts what it saw in a captured array.
public class EventQueue {
    interface Handler {
        boolean wants(String kind);
        void handle(String kind, int value);
    }

    private Handler[] handlers = new Handler[2];
    private int count;
    private final StringBuilder log = new StringBuilder();
    private String[] kinds = new String[4];
    private int[] values = new int[4];
    private int queued;

    void register(Handler h) {
        if (count == handlers.length) {
            Handler[] bigger = new Handler[count * 2];
            for (int i = 0; i < count; i++) bigger[i] = handlers[i];
            handlers = bigger;
        }
        handlers[count++] = h;
    }

    void unregister(Handler h) {
        for (int i = 0; i < count; i++) {
            if (handlers[i] == h) {
                for (int j = i; j < count - 1; j++) handlers[j] = handlers[j + 1];
                handlers[--count] = null;
                return;
            }
        }
    }

    void post(String kind, int value) {
        if (queued == kinds.length) {
            String[] k = new String[queued * 2];
            int[] v = new int[queued * 2];
            for (int i = 0; i < queued; i++) {
                k[i] = kinds[i];
                v[i] = values[i];
            }
            kinds = k;
            values = v;
        }
        kinds[queued] = kind;
        values[queued++] = value;
    }

    void note(String s) { log.append(s).append('\n'); }

    void run() {
        for (int e = 0; e < queued; e++) {
            // A handler may unregister itself while the event is delivered: deliver to a copy.
            Handler[] now = new Handler[count];
            for (int i = 0; i < count; i++) now[i] = handlers[i];
            for (Handler h : now) {
                if (h.wants(kinds[e])) h.handle(kinds[e], values[e]);
            }
        }
        queued = 0;
    }

    public static void main(String[] args) {
        EventQueue q = new EventQueue();
        int[] totals = new int[2];
        String prefix = "> ";
        q.register(new Handler() {
            public boolean wants(String kind) { return true; }
            public void handle(String kind, int value) { q.note(prefix + kind + " " + value); }
        });
        q.register(new Handler() {
            int seen;
            public boolean wants(String kind) { return kind.equals("click"); }
            public void handle(String kind, int value) {
                seen++;
                q.note("click #" + seen + " at " + value);
                if (seen == 2) {
                    q.unregister(this);
                    q.note("click handler leaves");
                }
            }
        });
        q.register(new Handler() {
            public boolean wants(String kind) { return !kind.equals("click"); }
            public void handle(String kind, int value) { totals[kind.length() % 2] += value; }
        });
        q.post("click", 3);
        q.post("key", 65);
        q.post("click", 7);
        q.post("scroll", -2);
        q.post("click", 9);
        q.run();
        q.post("resize", 640);
        q.run();
        System.out.print(q.log);
        System.out.println("totals " + totals[0] + " " + totals[1] + ", handlers left " + q.count);
    }
}
