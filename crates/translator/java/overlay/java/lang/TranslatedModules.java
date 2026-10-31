package java.lang;

import java.io.IOException;
import java.io.InputStream;
import java.net.URL;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.cert.Certificate;
import java.util.ArrayList;
import java.util.Collections;
import java.util.Enumeration;
import java.util.IdentityHashMap;
import java.util.LinkedHashMap;
import java.util.concurrent.ConcurrentHashMap;
import sun.security.pkcs.PKCS7;
import sun.security.pkcs.SignerInfo;

/**
 * The translator's methods that calls of the JDK's class loading methods of {@code BootLoader},
 * {@code ClassLoaders} and {@code Reflection} call instead ({@code REDIRECTED_CALLS} in
 * {@code program/reach.rs}), and those that {@code Class.getModule()},
 * {@code Class.getClassLoader()}, {@code Class.getPackage()}, {@code Class.getSigners()},
 * {@code ClassLoader.getUnnamedModule()} and {@code Class.forName(module, name)} are ({@code MODULE_CALLS}, {@code lower/modules.rs}):
 * over the translation's modules ({@link Module}), whose classes are all the code there is.
 */
public final class TranslatedModules {
    /**
     * The packages asked for so far, by name: one {@code Package} each, as a class loader keeps
     * them, since a package is in one module of the translation. (A {@code LinkedHashMap}, so that
     * its calls reach only that class's methods, as in {@code Module}.)
     */
    private static final LinkedHashMap<String, Package> PACKAGES = new LinkedHashMap<>();

    /**
     * The values that {@code ClassLoaderValue}s keep, by class loader (the boot loader's under
     * {@code null}): the map that the JVM's loaders hold in a field each.
     */
    private static final IdentityHashMap<ClassLoader, ConcurrentHashMap<?, ?>> LOADER_VALUES =
            new IdentityHashMap<>();

    /** The certificates of {@link #signersOf}, by the blocks that gave them. */
    private static final LinkedHashMap<String, Certificate[]> SIGNERS = new LinkedHashMap<>();

    private TranslatedModules() {
    }

    /** {@code c.getModule()}. */
    public static Module moduleOf(Class<?> c) {
        return Module.of(c);
    }

    /** {@code c.getClassLoader()}: that of the class's module. */
    public static ClassLoader classLoaderOf(Class<?> c) {
        return Module.of(c).getClassLoader();
    }

    /**
     * {@code c.getPackage()}: null for an array, a primitive type and {@code void}, as in Java;
     * otherwise the package of the class's package name, {@code ""} for the unnamed package, in
     * the class's module ({@link Package#ofTranslation}).
     */
    public static synchronized Package packageOf(Class<?> c) {
        if (c.isArray() || c.isPrimitive()) {
            return null;
        }
        String name = c.getPackageName();
        Package p = PACKAGES.get(name);
        if (p == null) {
            p = Package.ofTranslation(name, Module.of(c));
            PACKAGES.put(name, p);
        }
        return p;
    }

    /**
     * {@code c.getSigners()}: the certificates of the signers of the jar that the class comes
     * from, where its root has the jar's signatures ({@code Module.signers0}): those of each
     * signer's chain, as Java's class loader takes them from the jar's code source
     * ({@code CodeSource.getCertificates}). Null for a class of no signed root, an array, a
     * primitive type and {@code void}, as in Java. Java verifies the jar's digests as it reads
     * the classes; a translation's classes come from their sources, so the signatures are taken
     * as the jar has them. A copy each call, of the certificates that the blocks gave the first
     * time.
     */
    public static synchronized Object[] signersOf(Class<?> c) {
        if (c.isArray() || c.isPrimitive() || Module.of(c).isNamed()) {
            return null;
        }
        String blocks = Module.signers0(c.getPackageName());
        if (blocks == null) {
            return null;
        }
        Certificate[] signers = SIGNERS.get(blocks);
        if (signers == null) {
            ArrayList<Certificate> certificates = new ArrayList<>();
            for (String block : blocks.split("\n")) {
                try {
                    PKCS7 signed = new PKCS7(Files.readAllBytes(Path.of(block)));
                    for (SignerInfo signer : signed.getSignerInfos()) {
                        certificates.addAll(signer.getCertificateChain(signed));
                    }
                } catch (IOException e) {
                    // Java's class loader drops a jar whose signatures it cannot read.
                    throw new SecurityException("the signature " + block + " of " + c.getName(), e);
                }
            }
            signers = certificates.toArray(new Certificate[0]);
            SIGNERS.put(blocks, signers);
        }
        return signers.clone();
    }

    /** {@code loader.getUnnamedModule()}. */
    public static Module unnamedModule(ClassLoader loader) {
        return Module.unnamedOf(loader);
    }

    /** {@code Class.forName(module, name)}, of the class {@code c} that the name finds, or null. */
    public static Class<?> inModule(Module module, Class<?> c) {
        return Module.within(c, module);
    }

    /** {@code BootLoader.getUnnamedModule()}. */
    public static Module bootUnnamedModule() {
        return Module.unnamedOf(null);
    }

    /** {@code BootLoader.loadClassOrNull(name)}: the class of a module of the boot loader, or null. */
    public static Class<?> bootClassOrNull(String name) {
        if (name == null) {
            // Java's fails so too, in the map of its class loading locks.
            throw new NullPointerException();
        }
        try {
            // (A concatenation, which is never null: the translation cannot give the message of
            // a `NullPointerException` in the code of `java.lang`, which `ResourceBundle` keeps.)
            return ofBootLoader(Class.forName("" + name, false, null));
        } catch (ClassNotFoundException e) {
            return null;
        }
    }

    // The class `c` if a module of the boot loader has it, or null.
    private static Class<?> ofBootLoader(Class<?> c) {
        Module module = Module.of(c);
        return module != null && module.getClassLoader() == null ? c : null;
    }

    /**
     * {@code BootLoader.findResource(name)}: a resource of a module of the boot loader, which is
     * not in the translation and throws; null for a name that none holds, as in Java.
     */
    public static URL bootResource(String name) {
        String module = Module.bootHolder(name);
        if (module != null) {
            throw new UnsupportedOperationException("the resource " + name + " of module " + module
                    + ": code that is not in the translation cannot run");
        }
        return null;
    }

    /**
     * {@code loader.getResources(name)}: nothing when the loader finds no resource of that name,
     * as in Java. The translation's loaders find a resource exactly when {@code getResource} does:
     * the system class loader's on the class path are each directory's that has it
     * ({@link Module#classPathResources}); any other throws as {@code getResource} does, and a
     * loader of the program's own that gives a URL throws here too, as its enumeration would need
     * the URLs of the loaders between.
     */
    public static Enumeration<URL> resources(ClassLoader loader, String name) throws IOException {
        URL url = loader.getResource(name);
        if (url == null) {
            return Collections.emptyEnumeration();
        }
        if (loader == ClassLoader.getSystemClassLoader()) {
            return Collections.enumeration(Module.classPathResources(name, true));
        }
        throw new UnsupportedOperationException("the resources " + name + " that " + loader
                + " finds, with " + url + ": code that is not in the translation cannot run");
    }

    /**
     * {@code c.getResource(name)}, as Java's: the name taken from the class's package unless it
     * starts with {@code /}, and found by the class's loader. A class file is not there, as the
     * program's classes were translated from their sources (javac's {@code -XDshowClass} asks
     * for one it cannot find). Nor is a resource of the JDK's modules as a URL: its data files,
     * such as {@code CharacterName}'s {@code uniName.dat}, are made when the JDK is built and are
     * not in its sources, so asking for one ends the program.
     */
    public static URL classResource(Class<?> c, String name) {
        String resolved = resolveName(c, name);
        return resolved.endsWith(".class") ? null : loaderOfResource(c, name).getResource(resolved);
    }

    /**
     * {@code c.getResourceAsStream(name)}: the class's loader's stream of the name, or null. A
     * data file of the JDK that code reads by a constant name from a class of the JDK's is the
     * one the translator read from the JDK ({@code Module.jdkResource0}).
     */
    public static InputStream classResourceAsStream(Class<?> c, String name) {
        String resolved = resolveName(c, name);
        if (resolved.endsWith(".class")) {
            return null;
        }
        ClassLoader loader = classLoaderOf(c);
        if (loader == null || loader == ClassLoader.getPlatformClassLoader()) {
            String data = Module.jdkResource0(resolved);
            if (data != null) {
                return new java.io.ByteArrayInputStream(data.getBytes(java.nio.charset.StandardCharsets.ISO_8859_1));
            }
        }
        return loaderOfResource(c, name).getResourceAsStream(resolved);
    }

    // The class loader that finds the resources of `c`, which is not one of the JDK's.
    private static ClassLoader loaderOfResource(Class<?> c, String name) {
        ClassLoader loader = classLoaderOf(c);
        if (loader == null || loader == ClassLoader.getPlatformClassLoader()) {
            throw new InternalError("the resource " + name + " of " + c.getName()
                    + " is not part of the translated program");
        }
        return loader;
    }

    // Java's `Class.resolveName`.
    private static String resolveName(Class<?> c, String name) {
        if (name.startsWith("/")) {
            return name.substring(1);
        }
        String base = c.getPackageName();
        return base.isEmpty() ? name : base.replace('.', '/') + "/" + name;
    }

    /** {@code ClassLoader.getSystemResources(name)}. */
    public static Enumeration<URL> systemResources(String name) throws IOException {
        return resources(ClassLoader.getSystemClassLoader(), name);
    }

    /** {@code ClassLoaders.appClassLoader()}. */
    public static ClassLoader appClassLoader() {
        return ClassLoader.getSystemClassLoader();
    }

    /** {@code ClassLoaders.platformClassLoader()}. */
    public static ClassLoader platformClassLoader() {
        return ClassLoader.getPlatformClassLoader();
    }

    /** {@code BootLoader.getClassLoaderValueMap()}. */
    public static ConcurrentHashMap<?, ?> bootClassLoaderValueMap() {
        return classLoaderValueMap(null);
    }

    /** The map of {@code ClassLoaderValue}s of {@code loader}, made as it is first asked for. */
    public static synchronized ConcurrentHashMap<?, ?> classLoaderValueMap(ClassLoader loader) {
        ConcurrentHashMap<?, ?> map = LOADER_VALUES.get(loader);
        if (map == null) {
            map = new ConcurrentHashMap<>();
            LOADER_VALUES.put(loader, map);
        }
        return map;
    }

    /** {@code Reflection.verifyModuleAccess(currentModule, memberClass)}, as Java's. */
    public static boolean verifyModuleAccess(Module currentModule, Class<?> memberClass) {
        Module memberModule = memberClass.getModule();
        if (currentModule == memberModule) {
            return true;
        }
        return memberModule.isExported(memberClass.getPackageName(), currentModule);
    }
}
