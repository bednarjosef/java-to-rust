import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.TimeUnit;

// Processes started with `ProcessBuilder` and `Runtime.exec` run for real: their standard
// streams are pipes to the program, they get the environment and the directory they are given,
// and their exit status is waited for. A program that does not exist fails to start with an
// `IOException`, which netty and JNA catch when they probe the system.
@SuppressWarnings("deprecation")
public class ProcessesRun {
    static String text(InputStream in) throws IOException {
        return new String(in.readAllBytes(), StandardCharsets.UTF_8).replace("\n", "\\n");
    }

    static void run(String what, ProcessBuilder builder) throws Exception {
        Process process = builder.start();
        String out = text(process.getInputStream());
        String err = text(process.getErrorStream());
        int status = process.waitFor();
        System.out.println(what + ": out [" + out + "] err [" + err + "] status " + status + " "
                + process.exitValue() + " alive " + process.isAlive());
    }

    public static void main(String[] args) throws Exception {
        run("echo", new ProcessBuilder("sh", "-c", "echo out; echo err >&2; exit 3"));
        run("merged", new ProcessBuilder("sh", "-c", "echo one; echo two >&2").redirectErrorStream(true));
        ProcessBuilder environment = new ProcessBuilder("sh", "-c", "echo $GREETING; pwd");
        environment.environment().put("GREETING", "hello from the parent");
        environment.directory(new File("/"));
        run("environment", environment);
        ProcessBuilder empty = new ProcessBuilder("/bin/sh", "-c", "echo [$HOME]");
        empty.environment().clear();
        run("empty environment", empty);

        Process cat = new ProcessBuilder("cat").start();
        try (OutputStream in = cat.getOutputStream()) {
            in.write("written to standard input\n".getBytes(StandardCharsets.UTF_8));
        }
        System.out.println("cat: " + text(cat.getInputStream()) + " " + cat.waitFor());

        Process exec = Runtime.getRuntime().exec(new String[] {"sh", "-c", "exit 7"});
        System.out.println("exec: " + exec.waitFor());
        Process split = Runtime.getRuntime().exec("echo  split\tat white space", null, new File("/"));
        System.out.println("exec of a line: " + text(split.getInputStream()) + " " + split.waitFor());

        Process sleeper = new ProcessBuilder("sleep", "30").start();
        System.out.println("sleeping: " + sleeper.isAlive() + " " + sleeper.waitFor(10, TimeUnit.MILLISECONDS));
        sleeper.destroy();
        System.out.println("destroyed: " + sleeper.waitFor() + " " + sleeper.isAlive());

        try {
            new ProcessBuilder("no-such-program-for-the-corpus").start();
        } catch (IOException e) {
            System.out.println("missing: " + e.getMessage());
        }
        try {
            Runtime.getRuntime().exec("");
        } catch (IllegalArgumentException e) {
            System.out.println("empty: " + e.getMessage());
        }
        try {
            new ProcessBuilder("sh", "-c", "true").directory(new File("/no/such/directory")).start();
        } catch (IOException e) {
            System.out.println("no directory: " + e.getMessage());
        }
        ProcessBuilder inherit = new ProcessBuilder("sh", "-c", "echo inherited").inheritIO();
        System.out.println("before the child");
        System.out.println("inherited: " + inherit.start().waitFor());
    }
}
