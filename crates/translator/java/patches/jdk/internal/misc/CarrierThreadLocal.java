package jdk.internal.misc;

/**
 * The translator's patch of the JDK's {@code CarrierThreadLocal} (see docs/jdk-overlays.md). The
 * JDK's keeps its value in the copy of the current thread's carrier thread, through the JVM's
 * {@code JavaLangAccess}; a translated program has no virtual threads, so every thread is its own
 * carrier, and the patched class keeps it in the current thread's copy, as {@code ThreadLocal}
 * does.
 */
class CarrierThreadLocal {
    @Edit(find = "JLA.getCarrierThreadLocal(this)", replace = "super.get()")
    public T get();

    @Edit(find = "JLA.setCarrierThreadLocal(this, value)", replace = "super.set(value)")
    public void set(T value);

    @Edit(find = "JLA.removeCarrierThreadLocal(this)", replace = "super.remove()")
    public void remove();

    @Remove
    private static final JavaLangAccess JLA;
}
