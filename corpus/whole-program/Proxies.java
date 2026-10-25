import java.io.IOException;
import java.lang.reflect.InvocationHandler;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;
import java.lang.reflect.UndeclaredThrowableException;
import java.util.Arrays;

// Proxies of lists of interfaces that the code spells out or passes as a `Class` parameter: the
// handler's calls with each method's `Method` and the boxed arguments, `Object`'s methods, the
// results cast and unboxed with Java's exceptions, an exception the method does not declare
// wrapped, the run-time names of the classes in the order they are defined, and the checks of
// the interfaces with Java's messages.
public class Proxies {
    public interface Greeter {
        String greet(String who);

        int count();

        default String hello() {
            return "hello from the interface";
        }
    }

    public interface Runner {
        void run() throws IOException;

        long size(int a, double b);
    }

    public interface Counter {
        int next(int step);
    }

    interface Hidden {
        char letter(char c);
    }

    static final ClassLoader LOADER = Proxies.class.getClassLoader();

    static class Handler implements InvocationHandler {
        int calls;

        public Object invoke(Object proxy, Method m, Object[] a) throws Throwable {
            calls++;
            System.out.println("invoke " + m + " " + Arrays.toString(a));
            switch (m.getName()) {
                case "greet":
                    return "hi " + a[0];
                case "count":
                    return null;
                case "hashCode":
                    return 7;
                case "equals":
                    return proxy == a[0];
                case "toString":
                    return "proxy " + calls;
                case "run":
                    if (calls % 2 == 0) {
                        throw new IOException("declared");
                    }
                    throw new Exception("not declared");
                case "size":
                    return "not a long";
                case "next":
                    return (Integer) a[0] + calls;
                case "letter":
                    return Character.toUpperCase((Character) a[0]);
                case "hello":
                    throw new IllegalStateException("unchecked");
                default:
                    return null;
            }
        }
    }

    static <T> T make(Class<T> type, InvocationHandler h) {
        return type.cast(Proxy.newProxyInstance(LOADER, new Class<?>[] {type}, h));
    }

    static void attempt(String what, Runnable action) {
        try {
            action.run();
            System.out.println(what + ": made");
        } catch (RuntimeException e) {
            System.out.println(what + ": " + e);
        }
    }

    public static void main(String[] args) throws Exception {
        Handler h = new Handler();
        Greeter g = (Greeter) Proxy.newProxyInstance(LOADER, new Class<?>[] {Greeter.class}, h);
        System.out.println(g.greet("bob"));
        Class<?> c = g.getClass();
        System.out.println(c.getName() + " " + c.getSimpleName() + " " + c.getPackageName());
        System.out.println(g.hashCode() + " " + g + " " + g.equals(g) + " " + g.equals("x"));
        try {
            g.count();
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
        try {
            g.hello();
        } catch (IllegalStateException e) {
            System.out.println(e);
        }

        Runner r = (Runner) Proxy.newProxyInstance(LOADER, new Class<?>[] {Runner.class, Greeter.class}, h);
        System.out.println(r.getClass().getName() + " " + (r instanceof Greeter) + " " + ((Greeter) r).greet("ann"));
        for (int i = 0; i < 2; i++) {
            try {
                r.run();
            } catch (UndeclaredThrowableException e) {
                System.out.println(e + " " + e.getCause());
            } catch (IOException e) {
                System.out.println("declared: " + e);
            }
        }
        try {
            r.size(1, 2.5);
        } catch (ClassCastException e) {
            System.out.println(e.getMessage());
        }

        Counter counter = make(Counter.class, h);
        System.out.println(counter.next(100) + " " + counter.getClass().getName());
        Greeter again = make(Greeter.class, h);
        System.out.println((again.getClass() == c) + " " + again.greet("cy"));
        System.out.println(Proxy.getProxyClass(LOADER, Counter.class) == counter.getClass());

        Hidden hidden = (Hidden) Proxy.newProxyInstance(LOADER, new Class<?>[] {Hidden.class}, h);
        System.out.println(hidden.letter('q') + " " + hidden.getClass().getName());

        System.out.println(Proxy.isProxyClass(c) + " " + Proxy.isProxyClass(Proxies.class) + " "
                + (Proxy.getInvocationHandler(g) == h));
        Method greet = Greeter.class.getMethod("greet", String.class);
        Method run = Runner.class.getMethod("run");
        System.out.println(Arrays.toString(greet.getExceptionTypes()) + " " + Arrays.toString(run.getExceptionTypes()));

        attempt("no handler", () -> Proxy.newProxyInstance(LOADER, new Class<?>[] {Greeter.class}, null));
        attempt("a class", () -> Proxy.newProxyInstance(LOADER, new Class<?>[] {String.class}, h));
        attempt("twice", () -> Proxy.newProxyInstance(LOADER, new Class<?>[] {Counter.class, Counter.class}, h));
        attempt("no array", () -> Proxy.newProxyInstance(LOADER, null, h));
        attempt("no interface", () -> Proxy.newProxyInstance(LOADER, new Class<?>[] {null}, h));
        attempt("not a proxy", () -> Proxy.getInvocationHandler("x"));
        System.out.println(h.calls);
    }
}
