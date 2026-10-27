import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;

// Classes found by names that only a resource file holds, as log4j's plugins are listed in the
// binary `Log4j2Plugins.dat` that its registry reads: no string literal and no reflection
// metadata names them, and the program refers to none of them.
public class Main {
    public static void main(String[] args) throws Exception {
        var stream = Main.class.getResourceAsStream("/META-INF/example/plugins.list");
        try (BufferedReader in = new BufferedReader(new InputStreamReader(stream, StandardCharsets.UTF_8))) {
            String line;
            while ((line = in.readLine()) != null) {
                if (line.isBlank() || line.startsWith("#")) {
                    continue;
                }
                Class<?> plugin = Class.forName(line.trim());
                Object made = plugin.getDeclaredConstructor().newInstance();
                Method apply = plugin.getMethod("apply", String.class);
                System.out.println(plugin.getName() + ": " + apply.invoke(made, "Hello"));
            }
        }
    }
}
