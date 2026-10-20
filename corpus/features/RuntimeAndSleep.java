public class RuntimeAndSleep {
    public static void main(String[] args) throws InterruptedException {
        // The memory figures are not the JVM's; these checks hold for both.
        long total = Runtime.getRuntime().totalMemory();
        long free = Runtime.getRuntime().freeMemory();
        long max = Runtime.getRuntime().maxMemory();
        System.out.println("free within total: " + (free <= total));
        System.out.println("total within max: " + (total <= max));
        System.out.println("free is positive: " + (free > 0));
        System.out.println("processors positive: " + (Runtime.getRuntime().availableProcessors() > 0));

        Runtime.getRuntime().gc();
        System.out.println("collected");

        // Sleeps last at least as long as they were asked to.
        long start = System.currentTimeMillis();
        Thread.sleep(30);
        long slept = System.currentTimeMillis() - start;
        System.out.println("slept at least 30 ms: " + (slept >= 30));

        Thread.sleep(1, 500_000);
        Thread.sleep(0);
        Thread.onSpinWait();
        Thread.yield();
        System.out.println("spun and yielded");

        // Output before halt is still written, and the exit code is the one given.
        System.out.print("halting");
        Runtime.getRuntime().halt(3);
    }
}
