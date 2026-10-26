public class RuntimeLocal {
    public static void main(String[] args) {
        final Runtime runtime = Runtime.getRuntime();
        int processors = runtime.availableProcessors();
        long max = runtime.maxMemory();
        long total = runtime.totalMemory();
        long free = runtime.freeMemory();
        System.out.println("processors > 0: " + (processors > 0));
        System.out.println("max >= total: " + (max >= total));
        System.out.println("total >= free: " + (total >= free));
        Runnable report = () -> {
            Runtime inner = Runtime.getRuntime();
            System.out.println("inner processors > 0: " + (inner.availableProcessors() > 0));
        };
        report.run();
    }
}
