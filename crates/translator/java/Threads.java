// `java.lang.Thread` as the translator provides it: the constructors and methods that a
// translation can reproduce, written in the subset of Java the translator supports, over the
// natives of the runtime (`java_runtime::thread`). `build.rs` prints its syntax tree with
// `JavaTree`, and lowering gives it its real package and adds it, with the members a program uses,
// to the programs that use it, as it adds the standard exception classes of `Standard.java` (see
// `lower/exceptions.rs`). A program may extend it. Thread groups, interrupts, the thread's state
// and stack, and virtual threads are left out.
package threads;

class Thread {
    private String name;
    private final long tid;
    private final Runnable task;
    private boolean daemon;
    private int priority;
    private boolean started;

    public Thread() {
        this(null, genName());
    }

    public Thread(Runnable task) {
        this(task, genName());
    }

    public Thread(String name) {
        this(null, checkName(name));
    }

    public Thread(Runnable task, String name) {
        Thread parent = currentThread();
        this.task = task;
        this.name = checkName(name);
        this.tid = nextThreadId();
        this.daemon = parent.daemon;
        this.priority = parent.priority;
    }

    // The main thread, which `currentThread` makes when the main thread first asks for itself.
    private Thread(long tid) {
        this.task = null;
        this.name = "main";
        this.tid = tid;
        this.priority = 5;
    }

    private static String checkName(String name) {
        if (name == null) {
            throw new NullPointerException("'name' is null");
        }
        return name;
    }

    private static String genName() {
        return "Thread-" + nextThreadNum();
    }

    public static Thread currentThread() {
        if (!hasCurrent0()) {
            setCurrent0(new Thread(mainId0()));
        }
        return current0();
    }

    public void run() {
        if (task != null) {
            task.run();
        }
    }

    public void start() {
        if (started) {
            throw new IllegalThreadStateException();
        }
        started = true;
        startThread(this);
    }

    // The new thread makes `t` its current thread and runs `t.run()` (see `lower/natives.rs`).
    private static void startThread(Thread t) {
        start0(t, t.tid, t.name, t.daemon);
    }

    public final boolean isAlive() {
        return alive0(tid);
    }

    public final void join() throws InterruptedException {
        join0(tid, 0);
    }

    public final void join(long millis) throws InterruptedException {
        if (millis < 0) {
            throw new IllegalArgumentException("timeout value is negative");
        }
        join0(tid, millis);
    }

    public final String getName() {
        return name;
    }

    public final void setName(String name) {
        if (name == null) {
            throw new NullPointerException("name cannot be null");
        }
        this.name = name;
        rename0(tid, name);
    }

    public final boolean isDaemon() {
        return daemon;
    }

    public final void setDaemon(boolean on) {
        if (isAlive()) {
            throw new IllegalThreadStateException();
        }
        daemon = on;
    }

    public long getId() {
        return tid;
    }

    public final long threadId() {
        return tid;
    }

    public final int getPriority() {
        return priority;
    }

    public final void setPriority(int newPriority) {
        if (newPriority > 10 || newPriority < 1) {
            throw new IllegalArgumentException();
        }
        priority = newPriority;
    }

    // A thread that has ended has no group, whose name `main` the JVM shows otherwise.
    public String toString() {
        String group = started && !isAlive() ? "" : "main";
        return "Thread[#" + tid + "," + name + "," + priority + "," + group + "]";
    }

    private static native int nextThreadNum();

    private static native long nextThreadId();

    private static native long mainId0();

    private static native boolean hasCurrent0();

    private static native void setCurrent0(Thread t);

    private static native Thread current0();

    private static native void start0(Thread t, long tid, String name, boolean daemon);

    private static native boolean alive0(long tid);

    private static native void join0(long tid, long millis);

    private static native void rename0(long tid, String name);
}
