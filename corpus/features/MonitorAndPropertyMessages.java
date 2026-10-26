// The messages of NullPointerExceptions of the monitor methods of `Object` called on a null
// (`wait`, `wait(long)`, `notify`, `notifyAll`), and of `System.getProperty` of a key that is not
// a constant and is null, which `System.checkKey` throws with a message of its own.
public class MonitorAndPropertyMessages {
    Object lock;

    void signal() {
        lock.notifyAll();
    }

    void signalOne() {
        lock.notify();
    }

    void pause() throws InterruptedException {
        lock.wait();
    }

    void pauseFor(long millis) throws InterruptedException {
        lock.wait(millis);
    }

    static String property(String key) {
        return System.getProperty(key);
    }

    static String propertyOr(String key, String otherwise) {
        return System.getProperty(key, otherwise);
    }

    interface Action {
        void run() throws Exception;
    }

    static void report(Action action) {
        try {
            action.run();
            System.out.println("no exception");
        } catch (Exception e) {
            System.out.println(e.getClass().getSimpleName() + ": " + e.getMessage());
        }
    }

    public static void main(String[] args) {
        MonitorAndPropertyMessages m = new MonitorAndPropertyMessages();
        report(() -> m.signal());
        report(() -> m.signalOne());
        report(() -> m.pause());
        report(() -> m.pauseFor(5));
        report(() -> System.out.println(property(args.length > 0 ? args[0] : "line.separator").length()));
        report(() -> property(args.length > 0 ? args[0] : null));
        report(() -> System.out.println(propertyOr(args.length > 0 ? args[0] : "no.such.key", "none")));
        report(() -> propertyOr(args.length > 0 ? args[0] : null, "none"));
        m.lock = new Object();
        report(() -> {
            synchronized (m.lock) {
                m.signalOne();
                m.pauseFor(1);
            }
        });
    }
}
