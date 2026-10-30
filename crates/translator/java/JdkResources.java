// The data files of the JDK's modules that a translated program reads as resources of the JDK's
// classes (`program/reach/jdk_resources.rs`): files made when the JDK is built, which are not in
// its sources, such as the IDN profile `sun/net/idn/uidna.spp` that every HTTPS connection's host
// name goes through. For each resource name given, prints a line with the name, a tab and the
// bytes of the file of that name in this JDK's image in hex, or `-` where no module holds one.

import java.net.URI;
import java.nio.file.DirectoryStream;
import java.nio.file.FileSystem;
import java.nio.file.FileSystems;
import java.nio.file.Files;
import java.nio.file.Path;

public class JdkResources {
    public static void main(String[] args) throws Exception {
        FileSystem image = FileSystems.getFileSystem(URI.create("jrt:/"));
        StringBuilder out = new StringBuilder();
        for (String name : args) {
            byte[] data = null;
            try (DirectoryStream<Path> modules = Files.newDirectoryStream(image.getPath("/modules"))) {
                for (Path module : modules) {
                    Path file = module.resolve(name);
                    if (Files.isRegularFile(file)) {
                        data = Files.readAllBytes(file);
                        break;
                    }
                }
            }
            out.append(name).append('\t');
            if (data == null) {
                out.append('-');
            } else {
                for (byte b : data) {
                    out.append(Character.forDigit((b >> 4) & 15, 16)).append(Character.forDigit(b & 15, 16));
                }
            }
            out.append('\n');
        }
        System.out.print(out);
    }
}
