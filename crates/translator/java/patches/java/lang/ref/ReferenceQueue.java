package java.lang.ref;

/**
 * The translator's patch of the JDK's {@code ReferenceQueue} (see the patch of
 * {@code Reference}): {@code poll} first moves the references whose referents the collector
 * cleared onto their queues, and a thread that waits in {@code remove} starts the Reference
 * Handler thread, which moves them while it waits.
 */
class ReferenceQueue {
    @Edit(find = "if (head == null)", replace = """
            Reference.enqueuePendingReferences();
            if (head == null)""")
    public Reference<? extends T> poll();

    @Edit(find = "synchronized (lock) {", replace = """
            Reference.startReferenceHandler();
            Reference.enqueuePendingReferences();
            synchronized (lock) {""")
    public Reference<? extends T> remove(long timeout) throws InterruptedException;

    @Edit(find = "synchronized (lock) {", replace = """
            Reference.startReferenceHandler();
            Reference.enqueuePendingReferences();
            synchronized (lock) {""")
    public Reference<? extends T> remove() throws InterruptedException;
}
