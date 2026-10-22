package java.lang;

import java.net.URL;

/**
 * The translator's methods that calls of the JDK's class loading methods of {@code BootLoader},
 * {@code ClassLoaders} and {@code Reflection} call instead ({@code REDIRECTED_CALLS} in
 * {@code program/reach.rs}), and those that {@code Class.getModule()},
 * {@code Class.getClassLoader()}, {@code ClassLoader.getUnnamedModule()} and
 * {@code Class.forName(module, name)} are ({@code MODULE_CALLS}, {@code lower/modules.rs}):
 * over the translation's modules ({@link Module}), whose classes are all the code there is.
 */
public final class TranslatedModules {
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
        Class<?> c;
        try {
            c = Class.forName(name, false, null);
        } catch (ClassNotFoundException e) {
            return null;
        }
        return Module.of(c).getClassLoader() == null ? c : null;
    }

    /**
     * {@code BootLoader.findResource(name)}: a resource of a module of the boot loader, which is
     * not in the translation and throws; null for a name that none holds, as in Java.
     */
    public static URL bootResource(String name) {
        for (Module m : Module.named().values()) {
            if (m.getClassLoader() == null && Module.holds(m.getName(), name)) {
                throw new UnsupportedOperationException("the resource " + name + " of " + m
                        + ": code that is not in the translation cannot run");
            }
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
