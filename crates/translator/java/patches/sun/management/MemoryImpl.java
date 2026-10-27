package sun.management;

import java.lang.management.MemoryUsage;

/**
 * The translator's patch of the JDK's {@code MemoryImpl}, the memory figures of
 * {@code MemoryMXBean} (see docs/jdk-overlays.md).
 *
 * <p>The JVM's native sums its heap's or its other pools' usage. The heap here is the one
 * {@code Runtime} reports, with the same figures (which are not the collector's): it starts at
 * its total. Memory outside the heap is not counted, so it is none, with no limit, as the JVM's
 * metaspace has none.
 */
class MemoryImpl {
    @Replace
    private MemoryUsage getMemoryUsage0(boolean heap) {
        if (!heap) {
            return new MemoryUsage(0, 0, 0, -1);
        }
        Runtime runtime = Runtime.getRuntime();
        long total = runtime.totalMemory();
        return new MemoryUsage(total, total - runtime.freeMemory(), total, runtime.maxMemory());
    }
}
