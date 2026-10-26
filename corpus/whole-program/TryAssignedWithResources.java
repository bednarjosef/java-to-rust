import java.io.StringReader;
import java.io.StringWriter;
import java.io.Writer;
import java.util.ArrayList;
import java.util.List;

// Locals assigned in a `try` inside a try-with-resources, or in a try-with-resources itself, as the
// Minecraft server's `MinecraftServer.dumpNativeModules` and `PlayerSafetyServiceTextFilter` assign theirs.
public class TryAssignedWithResources {
    static List<String> list(boolean fail) {
        if (fail) throw new IllegalStateException("no");
        return List.of("b", "a");
    }

    static void dump(boolean fail) throws Exception {
        try (Writer output = new StringWriter()) {
            List<String> modules;
            try {
                modules = new ArrayList<>(list(fail));
            } catch (Throwable t) {
                System.out.println("failed " + t.getMessage());
                return;
            }
            modules.sort(null);
            for (String m : modules) {
                output.write(m);
            }
            System.out.println(output);
        }
    }

    static String cert(String text) {
        String certificate;
        try (StringReader in = new StringReader(text)) {
            if (text.isEmpty()) throw new IllegalArgumentException();
            certificate = "cert:" + (char) in.read();
        } catch (Exception e) {
            System.out.println("Failed to open certificate file");
            return null;
        }
        return certificate;
    }

    // A local of an object type, which has no value to start as, set in the try-with-resources:
    // the closing `try` with its `finally` carries it out.
    static String certObject(String text) {
        CharSequence certificate;
        try (StringReader in = new StringReader(text)) {
            if (text.isEmpty()) throw new IllegalArgumentException();
            certificate = new StringBuilder("object:").append((char) in.read());
        } catch (Exception e) {
            System.out.println("Failed to open certificate file");
            return null;
        }
        return certificate.toString();
    }

    public static void main(String[] args) throws Exception {
        dump(false);
        dump(true);
        System.out.println(cert("xyz"));
        System.out.println(cert(""));
        System.out.println(certObject("xyz"));
        System.out.println(certObject(""));
    }
}
