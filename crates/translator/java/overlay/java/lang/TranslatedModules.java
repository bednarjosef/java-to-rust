package java.lang;

import java.net.URL;
import java.util.LinkedHashMap;

/**
 * The translator's methods that calls of the JDK's class loading methods of {@code BootLoader},
 * {@code ClassLoaders} and {@code Reflection} call instead ({@code REDIRECTED_CALLS} in
 * {@code program/reach.rs}), and those that {@code Class.getModule()},
 * {@code Class.getClassLoader()}, {@code Class.getPackage()}, {@code ClassLoader.getUnnamedModule()}
 * and {@code Class.forName(module, name)} are ({@code MODULE_CALLS}, {@code lower/modules.rs}):
 * over the translation's modules ({@link Module}), whose classes are all the code there is.
 */
public final class TranslatedModules {
    /**
     * The packages asked for so far, by name: one {@code Package} each, as a class loader keeps
     * them, since a package is in one module of the translation. (A {@code LinkedHashMap}, so that
     * its calls reach only that class's methods, as in {@code Module}.)
     */
    private static final LinkedHashMap<String, Package> PACKAGES = new LinkedHashMap<>();

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

    /** {@code ClassLoaders.appClassLoader()}. */
    public static ClassLoader appClassLoader() {
        return ClassLoader.getSystemClassLoader();
    }

    /** {@code ClassLoaders.platformClassLoader()}. */
    public static ClassLoader platformClassLoader() {
        return ClassLoader.getPlatformClassLoader();
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
