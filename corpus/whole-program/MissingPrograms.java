import java.io.IOException;
import java.util.Arrays;

// A hardware library's way of asking the system's tools: a tool that is not installed is an
// `IOException`, caught with a `SecurityException` in one clause, under a `finally` that ends
// the process. The program reads no stream, so starting a program is all that raises the
// `IOException`.
public class MissingPrograms {
    static final String[] ENV = {"LC_ALL=C"};

    static int run(String command) {
        String[] cmd = command.split(" ");
        Process p = null;
        try {
            p = Runtime.getRuntime().exec(cmd, ENV);
            return p.waitFor();
        } catch (SecurityException | IOException e) {
            System.out.println("couldn't run " + Arrays.toString(cmd) + ": " + e.getMessage());
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        } finally {
            if (p != null) {
                p.destroy();
            }
        }
        return -1;
    }

    public static void main(String[] args) {
        System.out.println(run("true"));
        System.out.println(run("no-such-tool-for-the-corpus -v"));
        System.out.println(run("sh -c exit\t3"));
    }
}
