import java.lang.reflect.Proxy;

// Whether a class is a proxy's, and asking a non-proxy for its handler, as a library asks of an
// object it may have made as a proxy.
public class ProxyClassTests {
    static class Plain implements Runnable {
        public void run() {
        }
    }

    static String handlerOf(Object o) {
        try {
            return String.valueOf(Proxy.getInvocationHandler(o));
        } catch (IllegalArgumentException e) {
            return e.toString();
        } catch (NullPointerException e) {
            return "NullPointerException: " + e.getMessage();
        }
    }

    public static void main(String[] args) {
        Runnable r = new Plain();
        System.out.println(Proxy.isProxyClass(r.getClass()));
        System.out.println(Proxy.isProxyClass(String.class));
        try {
            Proxy.isProxyClass(null);
        } catch (NullPointerException e) {
            System.out.println("NullPointerException: " + e.getMessage());
        }
        System.out.println(handlerOf(r));
        System.out.println(handlerOf("text"));
        System.out.println(handlerOf(null));
    }
}
