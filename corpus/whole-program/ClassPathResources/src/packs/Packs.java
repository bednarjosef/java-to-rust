package packs;

import java.io.IOException;
import java.io.InputStream;
import java.net.URI;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.Collections;
import java.util.Enumeration;
import java.util.List;
import java.util.stream.Stream;

// Files on the class path next to the program's classes, read as a game reads its data packs: a
// file by name from the root and from the class's package, the URL of a marker file turned into
// a path whose directory is then walked, and a directory found through the class loader.
public class Packs {
    static String read(InputStream in) throws IOException {
        try (in) {
            return new String(in.readAllBytes(), StandardCharsets.UTF_8).trim();
        }
    }

    public static void main(String[] args) throws Exception {
        System.out.println(read(Packs.class.getResourceAsStream("/version.json")));
        System.out.println(read(Packs.class.getResourceAsStream("near.txt")));
        System.out.println(read(ClassLoader.getSystemResourceAsStream("packs/data/stone/color.txt")));
        System.out.println(Packs.class.getResourceAsStream("missing.txt") == null);
        System.out.println(Packs.class.getResource("/no/such/file") == null);
        System.out.println(Packs.class.getResource("../version.json") == null);

        URL marker = Packs.class.getResource("/packs/data/.root");
        URI uri = marker.toURI();
        System.out.println(uri.getScheme() + " " + marker.toString().endsWith("/packs/data/.root"));
        Path root = Paths.get(uri).getParent();
        System.out.println(root.getFileName());
        try (Stream<Path> files = Files.walk(root)) {
            files.filter(Files::isRegularFile)
                    .map(p -> root.relativize(p).toString().replace('\\', '/'))
                    .sorted()
                    .forEach(p -> System.out.println("  " + p));
        }
        System.out.println(Files.readString(root.resolve("stone/feel.txt")).trim());

        Enumeration<URL> dirs = Packs.class.getClassLoader().getResources("packs/data/");
        List<URL> found = Collections.list(dirs);
        System.out.println(found.size() + " " + found.get(0).toString().endsWith("/packs/data/"));
        URL dir = ClassLoader.getSystemResource("packs/data");
        System.out.println(dir.toString().endsWith("/packs/data"));
        System.out.println(Files.isDirectory(Paths.get(found.get(0).toURI())));
        System.out.println(Collections.list(ClassLoader.getSystemResources("nothing/here")).size());
    }
}
