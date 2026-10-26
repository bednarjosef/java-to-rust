import java.util.Map;
import java.util.Timer;
import java.util.TimerTask;
import java.util.TreeMap;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.ScheduledFuture;
import java.util.concurrent.TimeUnit;
import java.util.stream.Collectors;
import java.util.stream.Stream;

// A scheduled executor's one-shot and periodic tasks, a timer, a timed wait and parallel streams,
// where a catch prints what it catches: the executor queues its tasks through `super.getQueue()`,
// a call whose null result would be named by the superclass.
public class ScheduledExecutorCaught {
    public static void main(String[] args) {
        try {
            Object lock = new Object();
            synchronized (lock) {
                TimeUnit.MILLISECONDS.timedWait(lock, 1);
            }
            ScheduledExecutorService ses = Executors.newSingleThreadScheduledExecutor();
            ScheduledFuture<?> periodic = ses.scheduleAtFixedRate(() -> {}, 0, 1, TimeUnit.MILLISECONDS);
            ScheduledFuture<String> once = ses.schedule(() -> "once", 1, TimeUnit.MILLISECONDS);
            System.out.println(once.get());
            periodic.cancel(false);
            ses.shutdown();
            Timer timer = new Timer("timer", true);
            timer.schedule(new TimerTask() {
                public void run() {}
            }, 1);
            timer.cancel();
            Map<String, Integer> lengths = Stream.of("a", "bb", "cc").parallel()
                .collect(Collectors.toMap(s -> s, String::length));
            System.out.println(new TreeMap<>(lengths));
            Map<Integer, Integer> counts = Stream.of("a", "bb", "cc", "d").parallel()
                .collect(Collectors.toMap(String::length, s -> 1, Integer::sum));
            System.out.println(new TreeMap<>(counts));
        } catch (Exception e) {
            System.out.println(e.getMessage());
        }
    }
}
