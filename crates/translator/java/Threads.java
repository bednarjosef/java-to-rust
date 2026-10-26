// `java.lang.Thread` as the translator provides it: the constructors and methods that a
// translation can reproduce, written in the subset of Java the translator supports, over the
// natives of the runtime (`java_runtime::thread`). `build.rs` prints its syntax tree with
// `JavaTree`, and lowering gives it its real package and adds it, with the members a program uses,
// to the programs that use it, as it adds the standard exception classes of `Standard.java` (see
// `lower/exceptions.rs`). A program may extend it. The thread's stack has no frames, as an
// exception's has none; its state and virtual threads are left out. The interrupt status is the runtime's, by the thread's id, so that
// `sleep`, `join` and `wait` observe it. An uncaught exception goes to the thread's handler once
// the program has set a handler (`installUncaught0`); until then the runtime prints it, as the
// thread group would. `Runtime`'s shutdown hooks are kept here too.
package threads;

class Thread {
    public interface UncaughtExceptionHandler {
        void uncaughtException(Thread t, Throwable e);
    }

    private static UncaughtExceptionHandler defaultUncaughtExceptionHandler;
    private static final StackTraceElement[] EMPTY_STACK_TRACE = new StackTraceElement[0];

    public static final int MIN_PRIORITY = 1;
    public static final int NORM_PRIORITY = 5;
    public static final int MAX_PRIORITY = 10;

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
    // (and those of its `InheritableThreadLocal`s and `TerminatingThreadLocal`s). A new thread
    // copies its parent's inheritable ones (`inheritThreadLocals`).
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
        this(null, task, name, 0, true);
    }

    public Thread(ThreadGroup group, Runnable task) {
        this(group, task, genName());
    }

    public Thread(ThreadGroup group, String name) {
        this(group, null, checkName(name));
    }

    public Thread(ThreadGroup group, Runnable task, String name) {
        this(group, task, name, 0, true);
    }

    public Thread(ThreadGroup group, Runnable task, String name, long stackSize) {
        this(group, task, name, 0, true);
    }

    // The constructor of every new thread, and `ForkJoinWorkerThread`'s. As Java's, a thread that
    // does not inherit its parent's inheritable thread locals has the application's class loader.
    Thread(ThreadGroup group, Runnable task, String name, long stackSize, boolean inheritInheritableThreadLocals) {
        Thread parent = currentThread();
        this.task = task;
        this.name = checkName(name);
        this.tid = nextThreadId();
        this.daemon = parent.daemon;
        this.priority = parent.priority;
        this.group = group != null ? group : parent.group;
        if (inheritInheritableThreadLocals) {
            inheritThreadLocals(parent);
            this.contextClassLoader = parent.contextClassLoader;
        } else {
            this.contextClassLoader = ClassLoader.getSystemClassLoader();
        }
    }

    // The parent's inheritable thread locals, each value through its `childValue`, as the JDK's
    // constructor copies them. Only a program that sets one (`setInheritableThreadLocals`) has
    // this method; in any other, the call above does nothing (`lower/exceptions.rs`), so that a
    // program's threads do not bring in the JDK's `ThreadLocal`.
    private void inheritThreadLocals(Thread parent) {
        ThreadLocal.ThreadLocalMap parentMap = parent.inheritableThreadLocals;
        if (parentMap != null && parentMap.size() > 0) {
            inheritableThreadLocals = ThreadLocal.createInheritedMap(parentMap);
        }
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

    ThreadLocal.ThreadLocalMap inheritableThreadLocals() {
        return inheritableThreadLocals;
    }

    void setInheritableThreadLocals(ThreadLocal.ThreadLocalMap map) {
        inheritableThreadLocals = map;
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
        if (newPriority > MAX_PRIORITY || newPriority < MIN_PRIORITY) {
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

    // A thread's stack has no frames, as an exception's has none (see `Standard.java`), which the
    // specification of `getStackTrace` allows: Java's prints the exception without its frames.
    public static void dumpStack() {
        new Exception("Stack trace").printStackTrace();
    }

    // Empty, as an exception's (see `dumpStack`): a new array for a thread that is alive, and
    // Java's shared one for a thread that is not, as the JDK's.
    public StackTraceElement[] getStackTrace() {
        if (this != currentThread() && !isAlive()) {
            return EMPTY_STACK_TRACE;
        }
        return new StackTraceElement[0];
    }

    void dispatchUncaughtException(Throwable e) {
        getUncaughtExceptionHandler().uncaughtException(this, e);
    }

    // What the runtime runs, on the thread that an exception nothing catches ends, once the
    // program has set a handler: `uncaught0()` is that exception (see `lower/natives.rs`).
    private static void dispatchUncaught() {
        currentThread().dispatchUncaughtException(uncaught0());
    }

    // The hooks of `Runtime.addShutdownHook` and `removeShutdownHook`, whose calls are calls of
    // the methods below (`lower.rs`): threads that start as the program exits, which the exit
    // waits for. Null once that has begun.
    private static Thread[] shutdownHooks = new Thread[0];

    static synchronized void addShutdownHook(Thread hook) {
        if (shutdownHooks == null) {
            throw new IllegalStateException("Shutdown in progress");
        }
        if (hook.isAlive()) {
            throw new IllegalArgumentException("Hook already running");
        }
        for (Thread registered : shutdownHooks) {
            if (registered == hook) {
                throw new IllegalArgumentException("Hook previously registered");
            }
        }
        Thread[] hooks = new Thread[shutdownHooks.length + 1];
        System.arraycopy(shutdownHooks, 0, hooks, 0, shutdownHooks.length);
        hooks[shutdownHooks.length] = hook;
        shutdownHooks = hooks;
        installShutdown0();
    }

    static synchronized boolean removeShutdownHook(Thread hook) {
        if (shutdownHooks == null) {
            throw new IllegalStateException("Shutdown in progress");
        }
        if (hook == null) {
            throw new NullPointerException();
        }
        for (int i = 0; i < shutdownHooks.length; i++) {
            if (shutdownHooks[i] == hook) {
                Thread[] hooks = new Thread[shutdownHooks.length - 1];
                System.arraycopy(shutdownHooks, 0, hooks, 0, i);
                System.arraycopy(shutdownHooks, i + 1, hooks, i, hooks.length - i);
                shutdownHooks = hooks;
                return true;
            }
        }
        return false;
    }

    // The hooks, taken once: no hook can be added or removed after this.
    private static synchronized Thread[] takeShutdownHooks() {
        Thread[] hooks = shutdownHooks;
        shutdownHooks = null;
        return hooks;
    }

    // What the runtime runs, once, as the program exits: every hook starts, and the exit waits
    // until all of them have ended.
    private static void runShutdownHooks() {
        Thread[] hooks = takeShutdownHooks();
        for (Thread hook : hooks) {
            try {
                hook.start();
            } catch (IllegalThreadStateException startedAlready) {
            }
        }
        for (Thread hook : hooks) {
            boolean ended = false;
            while (!ended) {
                try {
                    hook.join();
                    ended = true;
                } catch (InterruptedException ignored) {
                }
            }
        }
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

    // Gives the runtime `runShutdownHooks()`, which it runs as the program exits (see
    // `lower/natives.rs`).
    private static native void installShutdown0();

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
// any other. Resources are not in the translation either: the bootstrap, platform and
// application loaders find the files and class files that the roots hold as Java's would, and a
// lookup that finds one throws, while one that finds none answers null or nothing, as in Java.
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

    // Java's asks its parent, or with none the bootstrap loader, then `findResource`.
    // `getResources` and `getSystemResources`, which give an `Enumeration` of the JDK, are
    // `TranslatedModules`' (`MODULE_CALLS`, `REDIRECTED_CALLS`).
    public java.net.URL getResource(String name) {
        if (name == null) {
            // Java's fails so, in `Objects.requireNonNull`.
            throw new NullPointerException();
        }
        java.net.URL url = parent != null ? parent.getResource(name) : held("bootstrap", name);
        return url != null ? url : findResource(name);
    }

    protected java.net.URL findResource(String name) {
        return this == SYSTEM || this == PLATFORM ? held(this.name, name) : null;
    }

    // Java's opens the stream of the URL that `getResource` finds, and gives null where that
    // fails: a file of the class path's (`Module.openFile`). Only a loader of the program that
    // overrides `getResource` or `findResource` can give another URL, whose stream the
    // translation does not have.
    public java.io.InputStream getResourceAsStream(String name) {
        java.net.URL url = getResource(name);
        if (url == null) {
            return null;
        }
        if (!url.getProtocol().equals("file")) {
            throw new UnsupportedOperationException("the stream of " + url
                    + ": code that is not in the translation cannot run");
        }
        try {
            return Module.openFile(url);
        } catch (java.io.IOException e) {
            return null;
        }
    }

    public static java.net.URL getSystemResource(String name) {
        return SYSTEM.getResource(name);
    }

    public static java.io.InputStream getSystemResourceAsStream(String name) {
        return SYSTEM.getResourceAsStream(name);
    }

    // The resource `name` that the loader `loader` (`bootstrap`, `platform` or `app`) finds, or
    // null for a name that it does not find, as in Java. A file of a directory of the class path
    // is read from there, as Java reads it; any other that the translator found in a root, a
    // module's resource or a class file, is not in the translation and throws.
    private static java.net.URL held(String loader, String name) {
        String holder = holder0(loader, name);
        if (loader.equals("app") && (holder == null || holder.equals("the class path"))) {
            java.util.ArrayList<java.net.URL> found = Module.classPathResources(name, false);
            if (!found.isEmpty()) {
                return found.get(0);
            }
        }
        if (holder != null) {
            throw new UnsupportedOperationException("the resource " + name + " of " + holder
                    + ": code that is not in the translation cannot run");
        }
        return null;
    }

    // The module that holds the resource `name` that the loader `loader` finds, as `module m` or
    // `the class path`, or null (see `lower/modules.rs`).
    private static native String holder0(String loader, String name);

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

    public final boolean parentOf(ThreadGroup g) {
        for (; g != null; g = g.parent) {
            if (g == this) {
                return true;
            }
        }
        return false;
    }

    // A thread group is never destroyed (Java's always answers so).
    public boolean isDestroyed() {
        return false;
    }

    public void uncaughtException(Thread t, Throwable e) {
        if (parent != null) {
            parent.uncaughtException(t, e);
        } else {
            Thread.UncaughtExceptionHandler ueh = Thread.getDefaultUncaughtExceptionHandler();
            if (ueh != null) {
                ueh.uncaughtException(t, e);
            } else {
                // Java prints the name as a string concatenation does, which takes a null.
                Thread.printUncaught0(String.valueOf(t.getName()), e);
            }
        }
    }

    public String toString() {
        return getClass().getName() + "[name=" + getName() + ",maxpri=" + getMaxPriority() + "]";
    }
}
