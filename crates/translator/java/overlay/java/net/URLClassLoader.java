package java.net;

import java.io.Closeable;
import java.io.File;
import java.io.IOException;

/**
 * The translator's {@code URLClassLoader}, which replaces the JDK's in a translation with the
 * JDK's sources (see docs/reflection-and-plugins.md).
 *
 * <p>A translated program is closed: it cannot load code that is not in the translation, and a
 * loader over URLs is the way Java loads such code (javac's annotation processors and plugins,
 * from the class path or the processor path). This loader finds what its parent finds, the
 * translation's classes, and nothing of its own: a class or a resource that is in one of its
 * directories, or a class asked of it when it has any other URL (a jar file's classes, which the
 * translation cannot read), fails loudly, saying what was asked for; one that is nowhere is not
 * found, as Java would not find it either. So {@code ServiceLoader} with this loader finds the
 * translation's providers when its directories declare none of the service, and fails loudly
 * when they do ({@code outside}); a jar file is not looked into for services.
 */
public class URLClassLoader extends ClassLoader implements Closeable {
    private final URL[] urls;

    public URLClassLoader(URL[] urls, ClassLoader parent) {
        super(parent);
        this.urls = urls.clone();
    }

    public URLClassLoader(URL[] urls) {
        super();
        this.urls = urls.clone();
    }

    public URLClassLoader(String name, URL[] urls, ClassLoader parent) {
        super(name, parent);
        this.urls = urls.clone();
    }

    public URL[] getURLs() {
        return urls.clone();
    }

    @Override
    protected Class<?> findClass(String name) throws ClassNotFoundException {
        String resource = name.replace('.', '/') + ".class";
        String where = outside(resource);
        for (int i = 0; where == null && i < urls.length; i++) {
            if (!isDirectory(urls[i])) {
                where = urls[i].toString();
            }
        }
        if (where != null) {
            throw new NoClassDefFoundError(name + " may be in " + where
                    + ", and code that is not in the translation cannot run");
        }
        throw new ClassNotFoundException(name);
    }

    public URL findResource(String name) {
        String where = outside(name);
        if (where != null) {
            throw new UnsupportedOperationException("the resource " + name + " is in " + where
                    + ", which is not in the translation");
        }
        return null;
    }

    /**
     * The directory among the URLs that has the resource {@code name}, which is not in the
     * translation, or null when none has it.
     */
    public String outside(String name) {
        for (int i = 0; i < urls.length; i++) {
            if (isDirectory(urls[i]) && new File(urls[i].getPath(), name).exists()) {
                return urls[i].getPath();
            }
        }
        return null;
    }

    private static boolean isDirectory(URL url) {
        return "file".equals(url.getProtocol()) && url.getPath().endsWith("/");
    }

    public void close() throws IOException {
    }
}
