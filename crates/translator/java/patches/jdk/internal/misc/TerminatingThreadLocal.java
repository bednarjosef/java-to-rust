package jdk.internal.misc;

import java.util.ArrayList;

/**
 * The translator's patch of the JDK's {@code TerminatingThreadLocal} (see docs/jdk-overlays.md).
 * Each thread's registry is a list that holds a thread-local once instead of a set over an
 * {@code IdentityHashMap}, whose comparison of keys by identity a translated program does not
 * have; a thread-local does not override {@code equals}, so the two hold the same thread-locals.
 */
class TerminatingThreadLocal {
    @Edit(find = "REGISTRY.get().add(tl);", replace = """
            Collection<TerminatingThreadLocal<?>> registered = REGISTRY.get();
            if (!registered.contains(tl)) {
                registered.add(tl);
            }
            """)
    public static void register(TerminatingThreadLocal<?> tl);

    @Edit(find = "Collections.newSetFromMap(new IdentityHashMap<>(4))", replace = "new ArrayList<>(4)")
    public static final TerminatingThreadLocal<Collection<TerminatingThreadLocal<?>>> REGISTRY;
}
