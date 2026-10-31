package java.lang.ref;

/**
 * The collector's registry of references, which the patched {@code Reference} uses (see
 * docs/collector.md, "References"): the runtime's {@code weak.rs} and {@code gc/references.rs}.
 */
final class TranslatedReferences {
    private TranslatedReferences() {
    }

    /**
     * Registers {@code reference} with the collector, which clears its referent once nothing
     * else keeps it: {@code kind} 1 for a soft reference, 2 for a weak one and 3 for a phantom
     * one; any other kind (a {@code FinalReference}) is not registered, and its referent stays as
     * long as the reference. {@code queued}: whether the reference has a queue, onto which it is
     * put once its referent is cleared ({@code next}).
     */
    static native void register(Reference<?> reference, int kind, boolean queued);

    /**
     * The next of the queued references whose referents the collector cleared, oldest first, or
     * {@code null} when there is none.
     */
    static native Reference<?> next();
}
