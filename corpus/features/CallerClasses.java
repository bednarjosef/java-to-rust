import java.util.function.Supplier;

// A static method that names the class of the code that called it, as logging libraries name a
// logger after the class that makes it.
public class CallerClasses {
    private static final StackWalker WALKER = StackWalker.getInstance(StackWalker.Option.RETAIN_CLASS_REFERENCE);

    static String caller() {
        return StackWalker.getInstance(StackWalker.Option.RETAIN_CLASS_REFERENCE).getCallerClass().getName();
    }

    static String callerThroughField(String what) {
        Class<?> c = WALKER.getCallerClass();
        return what + " from " + c.getSimpleName();
    }

    static class Service {
        static final String NAME = caller();

        String describe() {
            return callerThroughField("describe");
        }
    }

    interface Named {
        static String name() {
            return caller();
        }
    }

    enum Kind {
        A;

        String who() {
            return caller();
        }
    }

    public static void main(String[] args) {
        System.out.println(caller());
        System.out.println(Service.NAME);
        System.out.println(new Service().describe());
        System.out.println(Named.name());
        System.out.println(Kind.A.who());
        Supplier<String> lambda = () -> caller();
        System.out.println(lambda.get());
        Runnable anonymous = new Runnable() {
            public void run() {
                System.out.println(caller());
                System.out.println(callerThroughField("run"));
            }
        };
        anonymous.run();
        System.out.println(caller().equals(caller()));
    }
}
