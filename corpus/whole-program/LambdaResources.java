// try-with-resources over a lambda `AutoCloseable` (the JDK's interface): as an existing variable
// and as a declared resource initialized from it, closed once each, after the body.
public class LambdaResources {
    static final StringBuilder log = new StringBuilder();

    static int count(int n) throws Exception {
        int[] counter = {0};
        AutoCloseable tick = () -> log.append("closed after ").append(counter[0]).append("; ");
        try (tick) {
            for (int i = 0; i < n; i++) {
                counter[0]++;
            }
        }
        try (AutoCloseable again = tick) {
            counter[0] += 10;
        }
        return counter[0];
    }

    public static void main(String[] args) throws Exception {
        System.out.println(count(3) + " | " + log);
    }
}
