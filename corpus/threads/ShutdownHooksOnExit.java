public class ShutdownHooksOnExit {
    public static void main(String[] args) {
        Runtime.getRuntime().addShutdownHook(new Thread(() -> System.out.println("hook runs before the exit")));
        System.out.println("exiting with 4");
        System.exit(4);
        System.out.println("never printed");
    }
}
