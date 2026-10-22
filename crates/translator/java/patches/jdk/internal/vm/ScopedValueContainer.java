package jdk.internal.vm;

/**
 * The translator's patch of the JDK's {@code jdk.internal.vm.ScopedValueContainer} (see
 * docs/jdk-overlays.md), through which
 * {@code ScopedValue.Carrier} runs an operation with its values bound. The JDK's pushes a scope
 * when one of a {@code StructuredTaskScope} is on the thread's stack, and when the operation ends
 * checks that it closed the scopes it opened, then throws again what the operation threw (with
 * {@code Unsafe.throwException}). A translation has no {@code StructuredTaskScope}, a preview
 * API, and so never a scope on the stack: running the operation directly does what the JDK's does
 * then. For the same reason no thread is in a container that a thread owns, and {@code latest}
 * finds none.
 */
class ScopedValueContainer {
    @Replace
    public static <T extends ScopedValueContainer> T latest(Class<T> containerClass) {
        return null;
    }

    @Edit(find = "JLA.scopedValueBindings()", replace = "SharedSecrets.getJavaLangAccess().scopedValueBindings()")
    public static BindingsSnapshot captureBindings();

    @Replace
    public static void run(Runnable op) {
        op.run();
    }

    // What `op` throws goes on as it is, checked or not, as the JDK's `throwException` throws it:
    // the operation's exception type is only the compiler's.
    @Replace
    @SuppressWarnings("unchecked")
    public static <V, X extends Throwable> V call(CallableOp<V, X> op) {
        return ((CallableOp<V, RuntimeException>) (CallableOp<V, ?>) op).call();
    }

    // The JDK's makes sure that `StructureViolationException` is initialized, for the scopes
    // that a translation never has.
    static {}

    @Remove
    private static final JavaLangAccess JLA;

    @Remove
    private static void runWithoutScope(Runnable op);

    @Remove
    private void doRun(Runnable op);

    @Remove
    private static <V, X extends Throwable> V callWithoutScope(CallableOp<V, X> op);

    @Remove
    private <V, X extends Throwable> V doCall(CallableOp<V, X> op);

    @Remove
    private static void throwIfFailed(Throwable ex, boolean atTop);
}
