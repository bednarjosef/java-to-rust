import java.io.IOException;

// A local declared without a value and assigned in a `try` block, inside a `try` that holds the
// whole method, as the Minecraft server's `Main.main` loads its world data.
public class TryAssignedInsideOuterTry {
    static final class Data {
        final String text;

        Data(String text) {
            this.text = text;
        }
    }

    static Data load(String name) throws IOException {
        if (name.isEmpty()) {
            throw new IOException("no name");
        }
        return new Data("data of " + name);
    }

    static void run(String name) {
        try {
            System.out.println("loading " + name);
            Data data;
            if (name.length() < 10) {
                Data unfixed;
                try {
                    unfixed = load(name);
                } catch (IOException e) {
                    System.out.println("failed: " + e.getMessage());
                    return;
                }
                data = new Data(unfixed.text + " (fixed)");
            } else {
                data = null;
            }
            System.out.println(data == null ? "none" : data.text);
        } catch (RuntimeException e) {
            System.out.println("crashed: " + e);
        }
    }

    public static void main(String[] args) {
        run("world");
        run("");
        run("a very long name");
    }
}
