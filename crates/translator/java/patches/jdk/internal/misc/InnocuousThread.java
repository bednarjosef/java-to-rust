package jdk.internal.misc;

/**
 * The translator's patch of the JDK's {@code InnocuousThread} (see docs/jdk-overlays.md). Every
 * {@code FileChannelImpl} registers with the {@code Cleaner}, which starts this thread, and the
 * JDK's static initializer reads the fields of {@code Thread} and {@code ThreadGroup} with
 * {@code Unsafe}, which a translation has no field offsets for. The translator's {@code Thread} and
 * {@code ThreadGroup} have those fields as Java has them, so they are read directly here instead.
 */
class InnocuousThread {
    @Remove
    private static final jdk.internal.misc.Unsafe UNSAFE;
    @Remove
    private static final long THREAD_LOCALS;
    @Remove
    private static final long INHERITABLE_THREAD_LOCALS;
    @Remove
    private static final long CONTEXTCLASSLOADER;

    // The JDK stores the context class loader as a field of the new thread, bypassing the
    // override below, which refuses a non-null loader. The superclass's setter stores it the same way.
    @Edit(find = "UNSAFE.putReferenceRelease(this, CONTEXTCLASSLOADER, tccl);", replace = "super.setContextClassLoader(tccl);")
    private InnocuousThread(ThreadGroup group, Runnable target, String name, long stackSize, ClassLoader tccl);

    // The translator's threads keep their thread locals in fields that only java.lang can reach, so
    // this drops nothing. The cleaner thread calls it before each batch of cleanups; a cleanup that
    // reads a thread local it set in an earlier batch sees that value, where Java sees none.
    @Replace
    public final void eraseThreadLocals() {
    }

    // The topmost thread group is the parent of the new group, found through `getParent` in place
    // of the JDK's Unsafe reads of the parent field.
    static {
        ThreadGroup group = Thread.currentThread().getThreadGroup();

        while (group != null) {
            ThreadGroup parent = group.getParent();
            if (parent == null)
                break;
            group = parent;
        }
        INNOCUOUSTHREADGROUP = new ThreadGroup(group, "InnocuousThreadGroup");
    }
}
