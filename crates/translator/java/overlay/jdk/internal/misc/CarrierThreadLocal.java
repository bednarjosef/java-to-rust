package jdk.internal.misc;

/**
 * The translator's {@code CarrierThreadLocal}, which replaces the JDK's in a translation with the
 * JDK's sources. The JDK's keeps its value in the copy of the current thread's carrier thread,
 * through the JVM's {@code JavaLangAccess}; a translated program has no virtual threads, so every
 * thread is its own carrier, and this one keeps it in the current thread's copy, as
 * {@code ThreadLocal} does.
 */
public class CarrierThreadLocal<T> extends ThreadLocal<T> {
    @Override
    public T get() {
        return super.get();
    }

    @Override
    public void set(T value) {
        super.set(value);
    }

    @Override
    public void remove() {
        super.remove();
    }
}
