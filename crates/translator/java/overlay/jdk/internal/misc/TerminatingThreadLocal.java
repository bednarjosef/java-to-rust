package jdk.internal.misc;

import java.util.ArrayList;
import java.util.Collection;

/**
 * The translator's {@code TerminatingThreadLocal}, which replaces the JDK's in a translation with
 * the JDK's sources. It is the JDK's, except that each thread's registry is a list that holds a
 * thread-local once instead of a set over an {@code IdentityHashMap}, whose comparison of keys by
 * identity a translated program does not have; a thread-local does not override {@code equals},
 * so the two hold the same thread-locals.
 */
public class TerminatingThreadLocal<T> extends CarrierThreadLocal<T> {

    @Override
    public void set(T value) {
        super.set(value);
        register(this);
    }

    @Override
    public void remove() {
        unregister(this);
        super.remove();
    }

    /**
     * Invoked by a thread when terminating and this thread-local has an associated
     * value for the terminating thread (even if that value is null), so that any
     * native resources maintained by the value can be released.
     *
     * @param value current thread's value of this thread-local variable
     *              (may be null but only if null value was explicitly initialized)
     */
    protected void threadTerminated(T value) {
    }

    /**
     * Invokes the TerminatingThreadLocal's {@link #threadTerminated()} method
     * on all instances registered in current thread.
     */
    public static void threadTerminated() {
        for (TerminatingThreadLocal<?> ttl : REGISTRY.get()) {
            ttl._threadTerminated();
        }
    }

    private void _threadTerminated() { threadTerminated(get()); }

    /**
     * Register given TerminatingThreadLocal
     *
     * @param tl the ThreadLocal to register
     */
    public static void register(TerminatingThreadLocal<?> tl) {
        if (tl != REGISTRY) {
            Collection<TerminatingThreadLocal<?>> registered = REGISTRY.get();
            if (!registered.contains(tl)) {
                registered.add(tl);
            }
        }
    }

    /**
     * Unregister given TerminatingThreadLocal
     *
     * @param tl the ThreadLocal to unregister
     */
    private static void unregister(TerminatingThreadLocal<?> tl) {
        REGISTRY.get().remove(tl);
    }

    /**
     * A per-platform-thread registry of TerminatingThreadLocal(s). The registry is
     * itself a TerminatingThreadLocal to keep it reachable until the thread terminates.
     */
    public static final TerminatingThreadLocal<Collection<TerminatingThreadLocal<?>>> REGISTRY =
        new TerminatingThreadLocal<>() {
            @Override
            protected Collection<TerminatingThreadLocal<?>> initialValue() {
                return new ArrayList<>(4);
            }
        };
}
