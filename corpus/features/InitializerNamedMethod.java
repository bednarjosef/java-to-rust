// A class whose static method is named as the function that runs the class's static
// initialization (`initLogManager()` beside `LogManager`'s `init_log_manager`), as
// java.util.logging's LogManager has one.
public class InitializerNamedMethod {
    public static void main(String[] args) {
        System.out.println(LogManager.manager.name);
        System.out.println(LogManager.initLogManager().name + " " + LogManager.made);
        System.out.println(LogManager.initLogManagerSlow().name);
    }
}

class LogManager {
    static final LogManager manager = initLogManager();
    static int made;

    final String name;

    LogManager(String name) {
        this.name = name;
    }

    static LogManager initLogManager() {
        made++;
        return new LogManager("manager " + System.nanoTime() % 1);
    }

    static LogManager initLogManagerSlow() {
        return new LogManager("slow");
    }
}
