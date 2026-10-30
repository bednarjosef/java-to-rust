// `Integer.getInteger` and `Long.getLong` of a key that code computes, as `KeepAliveCache` reads
// `"http.keepAlive.time." + type` in its static initializer: a property of a plain run that is a
// number gives it, any other key (and a null one) the default, and the key and the default are
// evaluated once each, in that order. A constant key gives a constant default, unboxed as
// `KeepAliveCache` reads it.
public class ComputedNumberProperties {
    static final int MAX_CONNECTIONS = 5;
    static final long MAX_WAIT = 30;
    static int calls;
    static int result = -1;

    static int seconds(String type) {
        int v = Integer.getInteger("http.keepAlive.time." + type, -1);
        return v < -1 ? -1 : v;
    }

    static String key(String key) {
        calls = calls * 10 + 1;
        return key;
    }

    static int fallback() {
        calls = calls * 10 + 2;
        return 5;
    }

    public static void main(String[] args) {
        System.out.println(seconds("server") + " " + seconds("proxy"));
        String[] keys = {"sun.arch.data.model", "java.specification.version", "os.name", "no.such.key", null, ""};
        for (String k : keys) {
            System.out.println(k + " " + Integer.getInteger(k) + " " + Integer.getInteger(k, 7) + " "
                + Long.getLong(k) + " " + Long.getLong(k, 8L));
        }
        Integer read = Integer.getInteger(key("sun.arch.data.model"), fallback());
        System.out.println(read + " " + calls);
        calls = 0;
        Long missing = Long.getLong(key("no.such.key"), fallback());
        System.out.println(missing + " " + calls);
        result = Integer.getInteger("http.maxConnections", MAX_CONNECTIONS);
        long wait = Long.getLong("no.such.wait", MAX_WAIT);
        System.out.println(result + " " + wait);
    }
}
