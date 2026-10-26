import java.lang.reflect.InvocationHandler;
import java.lang.reflect.Proxy;
import java.util.Arrays;

// A proxy in a program whose `Class[][]` is also an `Object[]`: the translator's lists of the
// proxy classes' interfaces are a `Class[][]` too, made as the program holds one.
public class ProxiesBesideClassArrays {
    interface Greeter {
        String greet(String name);
    }

    public static void main(String[] args) {
        Object[] rows = new Class<?>[][] {{Greeter.class}, {Runnable.class, Comparable.class}};
        System.out.println(rows.length + " " + Arrays.toString((Class<?>[]) rows[1]));
        InvocationHandler h = (proxy, method, arguments) -> method.getName() + " " + arguments[0];
        Greeter g = (Greeter) Proxy.newProxyInstance(
                ProxiesBesideClassArrays.class.getClassLoader(), new Class<?>[] {Greeter.class}, h);
        System.out.println(g.greet("ada"));
        System.out.println(Proxy.isProxyClass(g.getClass()));
    }
}
