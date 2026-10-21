// A thread reads the command-line arguments that main was given, while main reads them too. The
// program starts threads, so its arrays are shared between them: the arguments must come out the
// same in both.
public class MainArgumentsThreads {
    static String[] seen;

    public static void main(String[] args) throws InterruptedException {
        seen = args;
        Thread worker = new Thread(() -> {
            int total = 0;
            for (String arg : seen) {
                total += arg.length();
            }
            System.out.println("worker: " + seen.length + " arguments, " + total + " characters");
        });
        worker.start();
        worker.join();
        System.out.println("main: " + args[0] + " " + args[args.length - 1]);
        args[0] = "first";
        System.out.println("seen: " + seen[0]);
    }
}
