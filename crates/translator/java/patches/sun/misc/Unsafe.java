package sun.misc;

/**
 * The translator's patch of the JDK's {@code sun.misc.Unsafe} (see docs/jdk-overlays.md), whose
 * class initialization a program runs once it holds the object. A call of one of its methods that
 * reach memory is the translator's access after its warning (see {@code lower/sun_unsafe.rs}), so
 * what remains to translate is the initialization, and the flag that the warning sets is a plain
 * static field.
 */
@Edit(find = "Reflection.registerMethodsToFilter(Unsafe.class, Set.of(\"getUnsafe\"));", replace = "")
@Edit(from = "private static final Object MEMORY_ACCESS_WARNED_BASE;",
        to = "throw new ExceptionInInitializerError(e); } }", replace = "")
class Unsafe {
    // Reflection's table leaves out `getUnsafe` as the filter that the static initializer
    // registers would (`lower/reflection.rs`). The flag is read and set without its base and
    // offset, which the static initializer found through reflection.
    @Replace
    private static boolean isMemoryAccessWarned() {
        return memoryAccessWarned;
    }

    @Replace
    private static boolean trySetMemoryAccessWarned() {
        synchronized (Unsafe.class) {
            if (memoryAccessWarned) {
                return false;
            }
            memoryAccessWarned = true;
            return true;
        }
    }
}
