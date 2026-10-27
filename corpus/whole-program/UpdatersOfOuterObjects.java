import java.util.concurrent.atomic.AtomicIntegerFieldUpdater;

// A field updater of the enclosing object (`Executor.this`) with constants for its states, as
// netty's `SingleThreadEventExecutor.doStartThread` suspends its loop from the `Runnable` that
// runs it.
public class UpdatersOfOuterObjects {
    static final class Executor {
        static final int ST_STARTED = 1;
        static final int ST_SUSPENDING = 2;
        static final int ST_SUSPENDED = 3;

        private static final AtomicIntegerFieldUpdater<Executor> STATE_UPDATER =
                AtomicIntegerFieldUpdater.newUpdater(Executor.class, "state");

        private volatile int state = ST_STARTED;
        private final StringBuilder log = new StringBuilder();

        Runnable loop() {
            return new Runnable() {
                @Override
                public void run() {
                    for (int round = 0; round < 3; round++) {
                        log.append(round).append(':').append(state).append(' ');
                        if (!STATE_UPDATER.compareAndSet(Executor.this, ST_SUSPENDING, ST_SUSPENDED)) {
                            STATE_UPDATER.compareAndSet(Executor.this, Executor.ST_STARTED, -ST_SUSPENDING + 4);
                            continue;
                        }
                        log.append("suspended");
                        return;
                    }
                }
            };
        }
    }

    public static void main(String[] args) {
        Executor executor = new Executor();
        executor.loop().run();
        System.out.println(executor.log + " " + executor.state);
    }
}
