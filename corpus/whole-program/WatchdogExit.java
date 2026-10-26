import java.util.Timer;
import java.util.TimerTask;

// A watchdog's way out, as the Minecraft server's `ServerWatchdog.exit` has it: a timer task
// that halts the process if `System.exit` hangs, and a halt where anything was thrown. Neither
// `halt` nor the task's run can throw, so a `catch (Throwable)` around them sees nothing.
public class WatchdogExit {
    static void exit(int status) {
        try {
            Timer timer = new Timer();
            timer.schedule(new TimerTask() {
                @Override
                public void run() {
                    Runtime.getRuntime().halt(2);
                }
            }, 10000L);
            System.out.println("exiting with " + status);
            System.exit(status);
        } catch (Throwable ignored) {
            Runtime.getRuntime().halt(status);
        }
    }

    public static void main(String[] args) {
        Runtime runtime = Runtime.getRuntime();
        runtime.addShutdownHook(new Thread(() -> System.out.println("hook ran")));
        System.out.println("watching");
        exit(3);
        System.out.println("not reached");
    }
}
