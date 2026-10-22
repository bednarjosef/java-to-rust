package java.net;

import java.io.File;

/**
 * The translator's patch of the JDK's {@code URLClassLoader} (see docs/jdk-overlays.md and
 * docs/reflection-and-plugins.md). It extends {@code ClassLoader}, not {@code SecureClassLoader}.
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
@Edit(find = "extends SecureClassLoader", replace = "extends ClassLoader")
class URLClassLoader {
    @Add
    private final URL[] urls;

    @Replace
    public URLClassLoader(URL[] urls, ClassLoader parent) {
        super(parent);
        this.urls = urls.clone();
    }

    @Replace
    public URLClassLoader(URL[] urls) {
        super();
        this.urls = urls.clone();
    }

    @Replace
    public URLClassLoader(String name, URL[] urls, ClassLoader parent) {
        super(name, parent);
        this.urls = urls.clone();
    }

    @Replace
    public URL[] getURLs() {
        return urls.clone();
    }

    @Replace
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

    @Replace
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
    @Add
    public String outside(String name) {
        for (int i = 0; i < urls.length; i++) {
            if (isDirectory(urls[i]) && new File(urls[i].getPath(), name).exists()) {
                return urls[i].getPath();
            }
        }
        return null;
    }

    @Add
    private static boolean isDirectory(URL url) {
        return "file".equals(url.getProtocol()) && url.getPath().endsWith("/");
    }

    @Replace
    public void close() throws IOException {
    }

    @Remove
    private final URLClassPath ucp;

    @Remove
    public URLClassLoader(URL[] urls, ClassLoader parent, URLStreamHandlerFactory factory);

    @Remove
    public URLClassLoader(String name, URL[] urls, ClassLoader parent, URLStreamHandlerFactory factory);

    @Remove
    private final WeakHashMap<Closeable,Void> closeables;

    @Remove
    public InputStream getResourceAsStream(String name);

    @Remove
    protected void addURL(URL url);

    @Remove
    private Package getAndVerifyPackage(String pkgname, Manifest man, URL url);

    @Remove
    private Class<?> defineClass(String name, Resource res) throws IOException;

    @Remove
    protected Package definePackage(String name, Manifest man, URL url);

    @Remove
    private boolean isSealed(String name, Manifest man);

    @Remove
    public Enumeration<URL> findResources(final String name) throws IOException;

    @Remove
    protected PermissionCollection getPermissions(CodeSource codesource);

    @Remove
    public static URLClassLoader newInstance(final URL[] urls, final ClassLoader parent);

    @Remove
    public static URLClassLoader newInstance(final URL[] urls);

    // The JDK's static initializer registers the class as parallel capable, which a translated
    // program's loaders have no use for.
    static {}
}
