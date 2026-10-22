package jdk.internal.vm;

import java.lang.ScopedValue.CallableOp;
import jdk.internal.access.SharedSecrets;

/**
 * The translator's {@code jdk.internal.vm.ScopedValueContainer}, through which
 * {@code ScopedValue.Carrier} runs an operation with its values bound. The JDK's pushes a scope
 * when one of a {@code StructuredTaskScope} is on the thread's stack, and when the operation ends
 * checks that it closed the scopes it opened, then throws again what the operation threw (with
 * {@code Unsafe.throwException}). A translation has no {@code StructuredTaskScope}, a preview
 * API, and so never a scope on the stack: running the operation directly does what the JDK's does
 * then. For the same reason no thread is in a container that a thread owns, and {@code latest}
 * finds none.
 */
public class ScopedValueContainer extends StackableScope {

    private ScopedValueContainer() {
    }

    public static <T extends ScopedValueContainer> T latest(Class<T> containerClass) {
        return null;
    }

    public static ScopedValueContainer latest() {
        return latest(ScopedValueContainer.class);
    }

    public record BindingsSnapshot(Object scopedValueBindings, ScopedValueContainer container) {
    }

    public static BindingsSnapshot captureBindings() {
        return new BindingsSnapshot(SharedSecrets.getJavaLangAccess().scopedValueBindings(), latest());
    }

    public static void run(Runnable op) {
        op.run();
    }

    // What `op` throws goes on as it is, checked or not, as the JDK's `throwException` throws it:
    // the operation's exception type is only the compiler's.
    @SuppressWarnings("unchecked")
    public static <V, X extends Throwable> V call(CallableOp<V, X> op) {
        return ((CallableOp<V, RuntimeException>) (CallableOp<V, ?>) op).call();
    }
}
