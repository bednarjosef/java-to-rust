// `Runtime.halt` ends the program at once: unlike `System.exit`, it runs no shutdown hook.
public class ShutdownHooksNotOnHalt {
    public static void main(String[] args) {
        Runtime.getRuntime().addShutdownHook(new Thread(() -> System.out.println("never printed: a hook")));
        System.out.println("halting with 3");
        Runtime.getRuntime().halt(3);
        System.out.println("never printed");
    }
}
