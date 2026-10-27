import java.io.File;
import java.nio.file.Files;
import java.nio.file.Path;

// `File.deleteOnExit`, whose files the JDK's own shutdown hook deletes, in the reverse order of
// the calls, after the program's hooks have ended: a hook of the program still sees them.
public class DeleteOnExit {
    public static void main(String[] args) throws Exception {
        Path dir = Files.createTempDirectory("delete-on-exit");
        File file = dir.resolve("note.txt").toFile();
        Files.writeString(file.toPath(), "kept until the exit");
        dir.toFile().deleteOnExit();
        file.deleteOnExit();
        Runtime.getRuntime().addShutdownHook(new Thread(() ->
                System.out.println("a hook sees the file: " + file.exists())));
        System.out.println("before the exit: " + file.exists());
    }
}
