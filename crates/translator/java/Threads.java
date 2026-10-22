// `java.lang.Thread` as the translator provides it: the constructors and methods that a
// translation can reproduce, written in the subset of Java the translator supports, over the
// natives of the runtime (`java_runtime::thread`). `build.rs` prints its syntax tree with
// `JavaTree`, and lowering gives it its real package and adds it, with the members a program uses,
// to the programs that use it, as it adds the standard exception classes of `Standard.java` (see
// `lower/exceptions.rs`). A program may extend it. The thread's state and stack, and virtual
// threads, are left out. The interrupt status is the runtime's, by the thread's id, so that
// `sleep`, `join` and `wait` observe it. An uncaught exception goes to the thread's handler once
// the program has set a handler (`installUncaught0`); until then the runtime prints it, as the
// thread group would.
package threads;

class Thread {
    public interface UncaughtExceptionHandler {
        void uncaughtException(Thread t, Throwable e);
    }

    private static UncaughtExceptionHandler defaultUncaughtExceptionHandler;

    private String name;
    private final long tid;
    private final Runnable task;
    private boolean daemon;
    private int priority;
    private boolean started;
    // The thread's group; null for the group `main`.
    private ThreadGroup group;
    private UncaughtExceptionHandler uncaughtExceptionHandler;
    // The fields of the JDK's `Thread` that `ThreadLocalRandom` and `LockSupport` read and write
    // through `Unsafe` at their offsets (see `lower/handles.rs`).
    long threadLocalRandomSeed;
    int threadLocalRandomProbe;
    int threadLocalRandomSecondarySeed;
    Object parkBlocker;
    // The values of the thread's `ThreadLocal`s, which the JDK's `ThreadLocal` keeps in the thread
    // (and those of its `TerminatingThreadLocal`s). The inheritable ones are only a field, which
    // `ForkJoinWorkerThread` clears: without the accessors of the JDK's `Thread`, and without a new
    // thread's copy of its parent's, `InheritableThreadLocal` is rejected rather than wrong.
    ThreadLocal.ThreadLocalMap threadLocals;
    ThreadLocal.ThreadLocalMap inheritableThreadLocals;
    ThreadLocal.ThreadLocalMap terminatingThreadLocals;
    // The thread's context class loader: what was set, the parent's when the thread was made, and
    // for the main thread the application's (`ClassLoader` below).
    private ClassLoader contextClassLoader;
    // The thread's scoped-value bindings, which the JDK's `ScopedValue` keeps in the thread. A
    // new thread has none, as a thread that no `StructuredTaskScope` starts. They start unknown
    // (null), where the JDK's start as `Thread.class` (`NEW_THREAD_BINDINGS`, none); the first
    // read finds none (`findScopedValueBindings`) and sets them, and so reads the same. (A field
    // that the constructors set would be every program's.)
    private Object scopedValueBindings;
    // The thread's cache of the scoped values it read last, which the JVM keeps for each thread
    // and `ScopedValue` fills and clears; empty (null) when the thread starts.
    private Object[] scopedValueCache;

    public Thread() {
        this((Runnable) null, genName());
    }

    public Thread(Runnable task) {
        this(task, genName());
    }

    public Thread(String name) {
        this((Runnable) null, checkName(name));
    }

    public Thread(Runnable task, String name) {
        Thread parent = currentThread();
        this.task = task;
        this.name = checkName(name);
        this.tid = nextThreadId();
        this.daemon = parent.daemon;
        this.priority = parent.priority;
        this.group = parent.group;
        this.contextClassLoader = parent.contextClassLoader;
    }

    public Thread(ThreadGroup group, Runnable task) {
        this(group, task, genName());
    }

    public Thread(ThreadGroup group, String name) {
        this(group, null, checkName(name));
    }

    public Thread(ThreadGroup group, Runnable task, String name) {
        this(task, name);
        if (group != null) {
            this.group = group;
        }
    }

    public Thread(ThreadGroup group, Runnable task, String name, long stackSize) {
        this(group, task, name);
    }

    // `ForkJoinWorkerThread`'s. A new thread copies none of its parent's inheritable thread
    // locals (see the fields above), so `inheritInheritableThreadLocals` changes nothing.
    Thread(ThreadGroup group, Runnable task, String name, long stackSize, boolean inheritInheritableThreadLocals) {
        this(group, task, name);
    }

    // The main thread, which `currentThread` makes when the main thread first asks for itself.
    private Thread(long tid) {
        this.task = null;
        this.name = "main";
        this.tid = tid;
        this.priority = 5;
        this.contextClassLoader = ClassLoader.getSystemClassLoader();
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
        // Started once the runtime has the thread's record, so that `getState` never finds a
        // started thread without one before it ends.
        startThread(this);
        started = true;
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

    public void interrupt() {
        interrupt0(tid);
    }

    public boolean isInterrupted() {
        return isInterrupted0(tid);
    }

    public static boolean interrupted() {
        return clearInterrupt0();
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

    ThreadLocal.ThreadLocalMap threadLocals() {
        return threadLocals;
    }

    void setThreadLocals(ThreadLocal.ThreadLocalMap map) {
        threadLocals = map;
    }

    ThreadLocal.ThreadLocalMap terminatingThreadLocals() {
        return terminatingThreadLocals;
    }

    void setTerminatingThreadLocals(ThreadLocal.ThreadLocalMap map) {
        terminatingThreadLocals = map;
    }

    public final boolean isDaemon() {
        return daemon;
    }

    /** A translated program's threads are all platform threads. */
    public enum State {
        NEW,
        RUNNABLE,
        BLOCKED,
        WAITING,
        TIMED_WAITING,
        TERMINATED
    }

    // What the runtime's record says (see `java_runtime::thread::State`), or without a record
    // whether the thread has started.
    public State getState() {
        switch (state0(tid)) {
            case 1:
                return State.RUNNABLE;
            case 2:
                return State.BLOCKED;
            case 3:
                return State.WAITING;
            case 4:
                return State.TIMED_WAITING;
            case 5:
                return State.TERMINATED;
            default:
                return started ? State.TERMINATED : State.NEW;
        }
    }

    public ClassLoader getContextClassLoader() {
        return contextClassLoader;
    }

    public void setContextClassLoader(ClassLoader cl) {
        contextClassLoader = cl;
    }

    public final boolean isVirtual() {
        return false;
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

    // A thread that has ended has no group.
    public String toString() {
        String groupName = "";
        if (!started || isAlive()) {
            // Read once, so that the test covers the call.
            ThreadGroup g = group;
            groupName = g != null ? g.getName() : "main";
        }
        return "Thread[#" + tid + "," + name + "," + priority + "," + groupName + "]";
    }

    public final ThreadGroup getThreadGroup() {
        if (started && !isAlive()) {
            return null;
        }
        return group != null ? group : ThreadGroup.mainGroup();
    }

    public void setUncaughtExceptionHandler(UncaughtExceptionHandler ueh) {
        uncaughtExceptionHandler = ueh;
        installUncaught0();
    }

    public UncaughtExceptionHandler getUncaughtExceptionHandler() {
        if (started && !isAlive()) {
            return null;
        }
        UncaughtExceptionHandler ueh = uncaughtExceptionHandler;
        if (ueh != null) {
            return ueh;
        }
        return getThreadGroup();
    }

    public static void setDefaultUncaughtExceptionHandler(UncaughtExceptionHandler ueh) {
        defaultUncaughtExceptionHandler = ueh;
        installUncaught0();
    }

    public static UncaughtExceptionHandler getDefaultUncaughtExceptionHandler() {
        return defaultUncaughtExceptionHandler;
    }

    void dispatchUncaughtException(Throwable e) {
        getUncaughtExceptionHandler().uncaughtException(this, e);
    }

    // What the runtime runs, on the thread that an exception nothing catches ends, once the
    // program has set a handler: `uncaught0()` is that exception (see `lower/natives.rs`).
    private static void dispatchUncaught() {
        currentThread().dispatchUncaughtException(uncaught0());
    }

    static Object scopedValueBindings() {
        return currentThread().scopedValueBindings;
    }

    static void setScopedValueBindings(Object bindings) {
        currentThread().scopedValueBindings = bindings;
    }

    // The JVM walks the stack for the bindings of a `ScopedValue.Carrier.runWith` when a thread's
    // are unknown (null). Here they are unknown only before the thread first reads them, when no
    // `runWith` has set any: none, which `ScopedValue` reads null as.
    static Object findScopedValueBindings() {
        return null;
    }

    static Object[] scopedValueCache() {
        return currentThread().scopedValueCache;
    }

    static void setScopedValueCache(Object[] cache) {
        currentThread().scopedValueCache = cache;
    }

    // The JVM makes the bindings visible to a stack walk; there is none here.
    static void ensureMaterializedForStackWalk(Object bindings) {
    }

    private static native int nextThreadNum();

    private static native long nextThreadId();

    private static native long mainId0();

    private static native boolean hasCurrent0();

    private static native void setCurrent0(Thread t);

    private static native Thread current0();

    private static native void start0(Thread t, long tid, String name, boolean daemon);

    private static native boolean alive0(long tid);

    private static native int state0(long tid);

    private static native void join0(long tid, long millis);

    private static native void rename0(long tid, String name);

    private static native void interrupt0(long tid);

    private static native boolean isInterrupted0(long tid);

    private static native boolean clearInterrupt0();

    private static native void installUncaught0();

    private static native Throwable uncaught0();

    static native void printUncaught0(String name, Throwable e);
}

// `java.lang.ClassLoader` over the translation's classes. A translated program is closed: the
// application's loader, which `getSystemClassLoader` returns and which loads the program's
// classes, finds a class by name as `Class.forName` does, and its parent, the platform's loader,
// finds nothing of its own. A loader of the program's own (`java.net.URLClassLoader`, or a
// subclass) asks its parent first and then its `findClass`, which finds nothing unless it
// overrides it: defining a class from bytes is code that is not in the translation, and is not
// here. `getClassLoader()` of a class is `of(c)`: null for the JDK's base classes, as Java's
// bootstrap loader is, the platform's for the APIs of `java.compiler`, the application's for
// any other. Resources are not in the translation either: a loader finds none.
class ClassLoader {
    private static final ClassLoader PLATFORM = new ClassLoader("platform", null, false);
    private static final ClassLoader SYSTEM = new ClassLoader("app", PLATFORM, true);

    private final String name;
    private final ClassLoader parent;
    // Whether it loads the translation's classes.
    private final boolean translated;

    private ClassLoader(String name, ClassLoader parent, boolean translated) {
        this.name = name;
        this.parent = parent;
        this.translated = translated;
    }

    protected ClassLoader(String name, ClassLoader parent) {
        this(name, parent, false);
    }

    protected ClassLoader(ClassLoader parent) {
        this(null, parent, false);
    }

    protected ClassLoader() {
        this(null, SYSTEM, false);
    }

    public static ClassLoader getSystemClassLoader() {
        return SYSTEM;
    }

    public static ClassLoader getPlatformClassLoader() {
        return PLATFORM;
    }

    public final ClassLoader getParent() {
        return parent;
    }

    public String getName() {
        return name;
    }

    public Class<?> loadClass(String name) throws ClassNotFoundException {
        return loadClass(name, false);
    }

    protected Class<?> loadClass(String name, boolean resolve) throws ClassNotFoundException {
        if (name == null) {
            // Java's fails so, without a message, in the map of its class loading locks.
            throw new NullPointerException();
        }
        if (parent != null) {
            try {
                return parent.loadClass(name, false);
            } catch (ClassNotFoundException e) {
                // Then this loader's own.
            }
        }
        return findClass(name);
    }

    protected Class<?> findClass(String name) throws ClassNotFoundException {
        if (translated) {
            return Class.forName(name, false, null);
        }
        throw new ClassNotFoundException(name);
    }

    public java.net.URL getResource(String name) {
        return null;
    }

    public java.io.InputStream getResourceAsStream(String name) {
        return null;
    }

    public static java.net.URL getSystemResource(String name) {
        return null;
    }

    public static java.io.InputStream getSystemResourceAsStream(String name) {
        return null;
    }

    public String toString() {
        return name == null ? "java.lang.ClassLoader" : "java.lang.ClassLoader[" + name + "]";
    }

    // `c.getClassLoader()`.
    static ClassLoader of(Class<?> c) {
        while (c.isArray()) {
            c = c.getComponentType();
        }
        String n = c.getName();
        if (c.isPrimitive() || n.startsWith("java.") || n.startsWith("sun.") || n.startsWith("jdk.internal.")) {
            return null;
        }
        if (n.startsWith("javax.tools.") || n.startsWith("javax.annotation.processing.")
                || n.startsWith("javax.lang.model.")) {
            return PLATFORM;
        }
        return SYSTEM;
    }
}

class ThreadGroup implements Thread.UncaughtExceptionHandler {
    private static final ThreadGroup MAIN = new ThreadGroup(new ThreadGroup(null, "system", 10), "main", 10);

    private final ThreadGroup parent;
    private final String name;
    private final int maxPriority;

    private ThreadGroup(ThreadGroup parent, String name, int maxPriority) {
        this.parent = parent;
        this.name = name;
        this.maxPriority = maxPriority;
    }

    public ThreadGroup(String name) {
        this(Thread.currentThread().getThreadGroup(), name);
    }

    public ThreadGroup(ThreadGroup parent, String name) {
        this(parent, name, maxPriority(parent));
    }

    // The JDK's constructor reads `parent.maxPriority`, and a null parent fails as it does there.
    private static int maxPriority(ThreadGroup parent) {
        if (parent == null) {
            throw new NullPointerException("Cannot read field \"maxPriority\" because \"parent\" is null");
        }
        return parent.maxPriority;
    }

    static ThreadGroup mainGroup() {
        return MAIN;
    }

    public final String getName() {
        return name;
    }

    public final ThreadGroup getParent() {
        return parent;
    }

    public final int getMaxPriority() {
        return maxPriority;
    }

    public void uncaughtException(Thread t, Throwable e) {
        if (parent != null) {
            parent.uncaughtException(t, e);
        } else {
            Thread.UncaughtExceptionHandler ueh = Thread.getDefaultUncaughtExceptionHandler();
            if (ueh != null) {
                ueh.uncaughtException(t, e);
            } else {
                Thread.printUncaught0(t.getName(), e);
            }
        }
    }

    public String toString() {
        return getClass().getName() + "[name=" + getName() + ",maxpri=" + getMaxPriority() + "]";
    }
}
