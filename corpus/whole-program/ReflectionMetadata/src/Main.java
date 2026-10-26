import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;

// Classes and methods found by names read from a file, which the reflection metadata of
// `META-INF/native-image` lists, as log4j-core finds its plugins: neither name is a string
// literal of the program.
public class Main {
    public static void main(String[] args) throws Exception {
        System.out.println("made before: " + plugins.Greeter.made() + " " + plugins.Shouter.made());
        var stream = Main.class.getResourceAsStream("/plugins.txt");
        try (BufferedReader in = new BufferedReader(new InputStreamReader(stream, StandardCharsets.UTF_8))) {
            String line;
            while ((line = in.readLine()) != null) {
                String[] parts = line.split(" ");
                Class<?> plugin = Class.forName(parts[0]);
                Object made = plugin.getDeclaredConstructor().newInstance();
                Method method = plugin.getMethod(parts[1], String.class);
                System.out.println(plugin.getSimpleName() + "." + method.getName() + ": " + method.invoke(made, "world"));
            }
        }
        System.out.println("made after: " + plugins.Greeter.made() + " " + plugins.Shouter.made());
    }
}
