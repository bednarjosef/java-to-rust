import java.io.IOException;
import java.net.URI;
import java.nio.file.DirectoryStream;
import java.nio.file.FileSystem;
import java.nio.file.FileSystems;
import java.nio.file.Files;
import java.nio.file.Path;

// The JDK's class library through the jrt file system, as javac reads it: the image
// `lib/modules` of the JDK that runs the program, a directory of a module listed, and a class
// file's first bytes read.
public class ClassBytesThroughJrt {
    public static void main(String[] args) throws IOException {
        FileSystem jrt = FileSystems.getFileSystem(URI.create("jrt:/"));
        Path lang = jrt.getPath("/modules/java.base/java/lang");
        int classes = 0;
        boolean object = false;
        try (DirectoryStream<Path> entries = Files.newDirectoryStream(lang)) {
            for (Path p : entries) {
                classes++;
                object |= p.getFileName().toString().equals("Object.class");
            }
        }
        System.out.println((classes > 100) + " " + object + " " + Files.isDirectory(lang));
        byte[] bytes = Files.readAllBytes(lang.resolve("Object.class"));
        System.out.printf("%02X%02X%02X%02X%n", bytes[0], bytes[1], bytes[2], bytes[3]);
        System.out.println(Files.exists(jrt.getPath("/modules/java.base/java/lang/NoSuchClass.class")));
    }
}
