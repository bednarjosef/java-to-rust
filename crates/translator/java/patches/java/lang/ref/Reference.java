package java.lang.ref;

/**
 * The translator's patch of the JDK's {@code Reference} (see docs/jdk-overlays.md and
 * docs/collector.md, "References"). A reference registers itself with the collector, which clears
 * the referent once nothing else keeps it and hands over the queued ones whose referents it
 * cleared ({@code TranslatedReferences}). No Reference Handler thread runs unless a thread waits on
 * a queue ({@code ReferenceQueue.remove}): {@code ReferenceQueue.poll} moves the cleared
 * references onto their queues itself ({@code enqueuePendingReferences}), so that a program that
 * only polls, as {@code WeakHashMap} does, starts no thread.
 */
class Reference {
    /**
     * Registers the reference: a {@code FinalReference} is not, as the translation runs no
     * finalizers, and a reference to {@code null} has nothing to clear.
     */
    @Edit(find = "this.queue = (queue == null) ? ReferenceQueue.NULL_QUEUE : queue;", replace = """
            this.queue = (queue == null) ? ReferenceQueue.NULL_QUEUE : queue;
            if (referent != null) {
                TranslatedReferences.register(this, this instanceof SoftReference ? 1
                        : this instanceof WeakReference ? 2 : this instanceof PhantomReference ? 3 : 0,
                        queue != null);
            }
            """)
    Reference(T referent, ReferenceQueue<? super T> queue);

    /** The JDK's, with the references taken from the collector one at a time. */
    @Replace
    private static void processPendingReferences() {
        waitForReferencePendingList();
        synchronized (processPendingLock) {
            processPendingActive = true;
        }
        enqueuePendingReferences();
        synchronized (processPendingLock) {
            processPendingActive = false;
            processPendingLock.notifyAll();
        }
    }

    /** Replaced by {@code TranslatedReferences.next}. */
    @Remove
    private static native Reference<?> getAndClearReferencePendingList();

    /**
     * Moves the cleared references onto their queues itself: the Reference Handler thread may
     * not run.
     */
    @Replace
    private static boolean waitForReferenceProcessing()
        throws InterruptedException
    {
        if (!hasReferencePendingList()) {
            synchronized (processPendingLock) {
                if (!processPendingActive) {
                    return false;
                }
                processPendingLock.wait();
                return true;
            }
        }
        enqueuePendingReferences();
        return true;
    }

    /** Whether the Reference Handler thread was started (`startReferenceHandler`). */
    @Add
    private static boolean handlerStarted;

    /**
     * Moves the references whose referents the collector cleared onto their queues, as the
     * Reference Handler thread does.
     */
    @Add
    static void enqueuePendingReferences() {
        for (Reference<?> ref = TranslatedReferences.next(); ref != null; ref = TranslatedReferences.next()) {
            ref.enqueueFromPending();
        }
    }

    /**
     * Starts the Reference Handler thread, in the root thread group as the JDK does, the first
     * time a thread waits on a queue.
     */
    @Add
    static void startReferenceHandler() {
        synchronized (processPendingLock) {
            if (handlerStarted) {
                return;
            }
            handlerStarted = true;
        }
        ThreadGroup tg = Thread.currentThread().getThreadGroup();
        for (ThreadGroup tgn = tg; tgn != null; tg = tgn, tgn = tg.getParent());
        startReferenceHandlerThread(tg);
    }
}
