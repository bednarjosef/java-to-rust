import java.security.Security;

// `System.getProperty` and `Security.getProperty` of a key that code computes, as
// `InetAddressCachePolicy` reads the lifetimes of its cache with a method of its own, and of
// constant keys: the properties of a plain run, and the JDK's security properties.
public class ComputedProperties {
    static String property(String key, String fallback) {
        return System.getProperty(key, fallback);
    }

    static String property(String key) {
        return System.getProperty(key);
    }

    static String security(String key) {
        return Security.getProperty(key);
    }

    // Whether a value is set, which is the same wherever the program runs, where the values of
    // `user.dir` and `java.home` are not.
    static String describe(String value) {
        return value == null ? "unset" : "set";
    }

    public static void main(String[] args) {
        String[] keys = {"file.separator", "path.separator", "line.separator", "user.dir", "java.home", "no.such.key"};
        for (String key : keys) {
            System.out.println(key + " = " + describe(property(key)) + ", with a default: "
                    + describe(property(key, "default")));
        }
        System.out.println(property("java.class.path") + " " + property("file.encoding")
                + " " + property("user.dir").equals(System.getProperty("user.dir"))
                + " " + property("user.dir").startsWith("/")
                + " " + property("file.separator").equals("/") + " " + property("line.separator").length());
        for (String key : new String[] {"networkaddress.cache.negative.ttl", "networkaddress.cache.ttl", "jdk.includeInExceptions", "securerandom.source", "no.such.key"}) {
            System.out.println(key + " = " + security(key));
        }
        System.out.println(Security.getProperty("networkaddress.cache.negative.ttl") + " "
                + Security.getProperty("no.such.key"));
    }
}
