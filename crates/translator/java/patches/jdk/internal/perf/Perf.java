package jdk.internal.perf;

import java.nio.ByteBuffer;
import java.nio.ByteOrder;

/**
 * The translator's patch of the JDK's {@code jdk.internal.perf.Perf} (see docs/jdk-overlays.md).
 * HotSpot creates each performance counter in shared memory that other processes can read; with
 * {@code -XX:-UsePerfData} it hands back a plain heap buffer instead. A translated program has no
 * other process to read the counters, so the counter is that buffer: 8 bytes in native order,
 * holding the initial value. {@code PerfCounter} reads and writes it through the buffer.
 */
class Perf {
    @Replace
    public ByteBuffer createLong(String name, int variability, int units, long value) {
        return ByteBuffer.allocate(8).order(ByteOrder.nativeOrder()).putLong(0, value);
    }
}
