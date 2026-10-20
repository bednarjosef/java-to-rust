package extra;

import java.util.function.Supplier;

/** Never used by the program: inheritance, interfaces and lambdas are not supported yet. */
public class Unused extends Thread implements Supplier<String> {
    @Override
    public String get() {
        Runnable task = () -> System.out.println("unused");
        task.run();
        return "unused";
    }
}
