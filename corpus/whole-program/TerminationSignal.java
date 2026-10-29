// SIGTERM ends a program that has a shutdown hook as System.exit does: the hook runs, on a
// thread of its own, and the status is 128 + 15.
public class TerminationSignal {
    public static void main(String[] args) throws Exception {
        Runtime.getRuntime().addShutdownHook(new Thread(() -> System.out.println("hook ran")));
        System.out.println("started");
        String pid = Long.toString(ProcessHandle.current().pid());
        new ProcessBuilder("kill", "-TERM", pid).start().waitFor();
        Thread.sleep(10_000);
        System.out.println("not reached");
    }
}
