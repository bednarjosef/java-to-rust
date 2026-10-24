import java.io.InputStream;
import java.net.URL;
import java.util.Collections;
import java.util.Enumeration;

// Resource lookups of the system and platform class loaders that find nothing: an empty
// enumeration, or null, as Java gives, and the class file of a class that is in no root.
public class ClassLoaderResources {
    public static void main(String[] args) throws Exception {
        ClassLoader app = ClassLoader.getSystemClassLoader();
        Enumeration<URL> none = app.getResources("no/such/file.txt");
        System.out.println(none.hasMoreElements() + " " + Collections.list(none));
        System.out.println(Collections.list(ClassLoader.getSystemResources("org/example/impl/Binder.class")));
        System.out.println(Collections.list(ClassLoader.getPlatformClassLoader().getResources("config.properties")));
        System.out.println(ClassLoader.getSystemResource("none.properties"));
        InputStream in = app.getResourceAsStream("none.properties");
        System.out.println(in == null);
        System.out.println(ClassLoader.getSystemResourceAsStream("data/none.bin") == null);
        // Java's platform loader does not see the class path.
        System.out.println(ClassLoader.getPlatformClassLoader().getResource("ClassLoaderResources.class"));
        try {
            app.getResources(null);
        } catch (NullPointerException e) {
            System.out.println("no name");
        }
    }
}
