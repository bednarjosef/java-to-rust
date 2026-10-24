// `System.getenv()`, the whole environment: one unmodifiable map, read once, whose entries agree
// with `System.getenv(name)` and whose iteration order a copy keeps, as a logging library copies
// it into its properties. What is printed does not depend on the environment's variables.
import java.util.*;

public class EnvironmentMap {
    public static void main(String[] args) {
        Map<String, String> environment = System.getenv();
        System.out.println(environment == System.getenv());

        boolean agree = true;
        List<String> order = new ArrayList<>();
        for (Map.Entry<String, String> e : environment.entrySet()) {
            agree &= e.getValue().equals(System.getenv(e.getKey()));
            agree &= environment.get(e.getKey()).equals(e.getValue());
            order.add(e.getKey());
        }
        System.out.println(agree + " " + (order.size() == environment.size()));
        System.out.println(new ArrayList<>(environment.keySet()).equals(order));
        System.out.println(environment.containsKey("NO_SUCH_VARIABLE_HERE") + " " + environment.get("NO_SUCH_VARIABLE_HERE"));

        Properties properties = new Properties();
        properties.putAll(environment);
        System.out.println(properties.size() == environment.size());
        Map<String, String> copy = new HashMap<>(environment);
        System.out.println(copy.equals(environment) + " " + environment.equals(copy));

        try {
            environment.put("NAME", "value");
        } catch (UnsupportedOperationException e) {
            System.out.println("unmodifiable put");
        }
        try {
            environment.clear();
        } catch (UnsupportedOperationException e) {
            System.out.println("unmodifiable clear");
        }
        try {
            environment.get(null);
        } catch (NullPointerException e) {
            System.out.println("null name");
        }
        try {
            environment.containsKey(3);
        } catch (ClassCastException e) {
            System.out.println("not a name");
        }
    }
}
