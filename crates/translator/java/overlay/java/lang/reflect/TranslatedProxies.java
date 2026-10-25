package java.lang.reflect;

import java.util.Arrays;
import java.util.HashMap;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;

/**
 * The translator's {@code Proxy.newProxyInstance}, {@code getProxyClass}, {@code isProxyClass}
 * and {@code getInvocationHandler}. The JVM defines a proxy class when a list of interfaces is
 * first asked for with a class loader; a translation has the proxy classes that the translator
 * wrote for the lists that reachable code can pass (see {@code program/reach/proxies.rs}), whose
 * lists, classes and constructors the natives give. Defining one is giving it its name, as
 * {@code Proxy.ProxyBuilder} does: {@code $ProxyN}, {@code N} counting the classes defined, in
 * the dynamic module {@code jdk.proxyK} that each class loader gets when it first needs one
 * ({@code com.sun.proxy.jdk.proxyK} when an interface's package is not exported), after the
 * checks of the interfaces that Java makes, in its order and with its messages; a class whose
 * interfaces are not all public is in their package, {@code p.$ProxyN}, as in Java. The dynamic
 * modules are not made: {@code getModule}, {@code getClassLoader} and {@code getPackage} of a
 * class in one throw ({@code Module.of}). A list without a class and a list asked for with a
 * second class loader fail loudly. Java makes annotations as proxies too, which the translation
 * makes as the patched {@code AnnotationInvocationHandler} itself (see
 * {@code lower/annotation_objects.rs}), so the numbers of the proxy classes defined after an
 * annotation is made are not known: asking such a class for its name fails (the runtime's
 * {@code java::name_proxy_class}).
 */
final class TranslatedProxies {
    private static final String ANNOTATION = "sun.reflect.annotation.AnnotationInvocationHandler";

    // `ProxyBuilder.nextUniqueNumber` and its `counter` of dynamic modules.
    private static long next;
    private static int modules;
    // The dynamic module of each class loader (`null` for the boot loader), the classes defined
    // for each list of interfaces, and the loader of each class defined.
    private static final Map<ClassLoader, String> dynamic = new HashMap<>();
    private static final Map<List<Class<?>>, Class<?>> defined = new HashMap<>();
    private static final Map<Class<?>, ClassLoader> loaders = new IdentityHashMap<>();

    private TranslatedProxies() {}

    static Object newProxyInstance(ClassLoader loader, Class<?>[] interfaces, InvocationHandler h) {
        Objects.requireNonNull(h);
        Class<?> c = proxyClass(loader, interfaces);
        return newInstance0(index(c), h);
    }

    static Class<?> getProxyClass(ClassLoader loader, Class<?>... interfaces) {
        return proxyClass(loader, interfaces);
    }

    static boolean isProxyClass(Class<?> cl) {
        // `Proxy.class.isAssignableFrom(cl)`, which throws without a message.
        if (cl == null) {
            throw new NullPointerException();
        }
        if (cl.getName().equals(ANNOTATION)) {
            throw new UnsupportedOperationException(
                    "the class of an annotation, which is not a proxy class in the translation");
        }
        synchronized (TranslatedProxies.class) {
            return loaders.containsKey(cl);
        }
    }

    static InvocationHandler getInvocationHandler(Object proxy) {
        if (!isProxyClass(proxy.getClass())) {
            throw new IllegalArgumentException("not a proxy instance");
        }
        return ((Proxy) proxy).h;
    }

    // `Proxy.getProxyConstructor` and `ProxyBuilder`: the class of the list, defined the first
    // time it is asked for.
    private static synchronized Class<?> proxyClass(ClassLoader loader, Class<?>[] interfaces) {
        // Java reads the length first.
        int length = interfaces.length;
        List<Class<?>> list = Arrays.asList(interfaces.clone());
        Class<?> found = defined.get(list);
        if (found != null) {
            if (loaders.get(found) != loader) {
                throw new UnsupportedOperationException("a proxy class of " + list
                        + " for a second class loader, which the translation does not make");
            }
            return found;
        }
        if (length > 65535) {
            throw new IllegalArgumentException("interface limit exceeded: " + length);
        }
        for (Class<?> intf : list) {
            if (intf == null) {
                throw new NullPointerException("Cannot invoke \"java.lang.Class.getMethods()\" because \"intf\" is null");
            }
        }
        IdentityHashMap<Class<?>, Boolean> seen = new IdentityHashMap<>();
        for (Class<?> intf : list) {
            if (!intf.isInterface()) {
                throw new IllegalArgumentException(intf.getName() + " is not an interface");
            }
            if (!visible(intf, loader)) {
                throw new IllegalArgumentException(intf.getName()
                        + " referenced from a method is not visible from class loader: " + loaderId(loader));
            }
            if (seen.put(intf, Boolean.TRUE) != null) {
                throw new IllegalArgumentException("repeated interface: " + intf.getName());
            }
        }
        int index = generated(list);
        if (index < 0) {
            throw new UnsupportedOperationException("a proxy class of " + list + ", which is not in the translation");
        }
        Class<?> c = classes0()[index];
        String hidden = packages0()[index];
        String name;
        String module;
        if (hidden != null) {
            // In the package and module of the interfaces that are not public.
            Class<?> intf = null;
            for (Class<?> i : list) {
                if (i.getPackageName().equals(hidden)) {
                    intf = i;
                }
            }
            if (intf.getClassLoader() != loader) {
                throw new IllegalArgumentException("non-public interface is not defined by the given loader");
            }
            name = (hidden.isEmpty() ? "" : hidden + ".") + "$Proxy" + next;
            Module m = intf.getModule();
            module = (m.isNamed() ? "module " + m.getName() : "unnamed module") + " of loader " + loaderName(loader);
        } else {
            boolean nonExported = false;
            for (Class<?> intf : list) {
                if (!intf.getModule().isExported(intf.getPackageName())) {
                    nonExported = true;
                }
            }
            String dynamicModule = dynamic.get(loader);
            if (dynamicModule == null) {
                modules++;
                dynamicModule = "jdk.proxy" + modules;
                dynamic.put(loader, dynamicModule);
            }
            name = (nonExported ? "com.sun.proxy." + dynamicModule : dynamicModule) + ".$Proxy" + next;
            module = "module " + dynamicModule + " of loader " + loaderName(loader);
        }
        next++;
        name0(c, annotationsMade0() ? "" : name, module, hidden == null);
        defined.put(list, c);
        loaders.put(c, loader);
        return c;
    }

    // Whether `Class.forName(c.getName(), false, loader)` finds `c`: its loader is `loader` or
    // one that `loader` delegates to.
    private static boolean visible(Class<?> c, ClassLoader loader) {
        ClassLoader own = c.getClassLoader();
        if (own == null) {
            return true;
        }
        for (ClassLoader at = loader; at != null; at = at.getParent()) {
            if (at == own) {
                return true;
            }
        }
        return false;
    }

    // `JavaLangAccess.getLoaderNameID`, for the loaders whose names the translation knows.
    private static String loaderId(ClassLoader loader) {
        if (loader == null) {
            return "null";
        }
        if (loader == ClassLoader.getPlatformClassLoader()) {
            return "'platform'";
        }
        if (loader == ClassLoader.getSystemClassLoader()) {
            return "'app'";
        }
        throw new UnsupportedOperationException(
                "the name of a class loader of the program's own, which the translation does not know");
    }

    // The loader as HotSpot's `ClassCastException` names it.
    private static String loaderName(ClassLoader loader) {
        return loader == null ? "'bootstrap'" : loaderId(loader);
    }

    // The index of the translator's class of the list, or -1.
    private static int generated(List<Class<?>> list) {
        Class<?>[][] lists = interfaces0();
        for (int i = 0; i < lists.length; i++) {
            if (lists[i].length == list.size()) {
                boolean same = true;
                for (int j = 0; j < lists[i].length; j++) {
                    same &= lists[i][j] == list.get(j);
                }
                if (same) {
                    return i;
                }
            }
        }
        return -1;
    }

    private static int index(Class<?> c) {
        Class<?>[] classes = classes0();
        for (int i = 0; i < classes.length; i++) {
            if (classes[i] == c) {
                return i;
            }
        }
        throw new IllegalStateException(c.getName());
    }

    // The lists of interfaces of the translator's proxy classes, the classes in the same order,
    // and a new object of the class at `index`, with handler `h`.
    private static native Class<?>[][] interfaces0();

    private static native Class<?>[] classes0();

    // For each class, the package of its interfaces that are not public, or null when all are.
    private static native String[] packages0();

    private static native Object newInstance0(int index, InvocationHandler h);

    // Gives the class its name, or none that is known for an empty one, its module and loader
    // as a `ClassCastException` names them, and whether that module is a dynamic one.
    private static native void name0(Class<?> c, String name, String module, boolean dynamic);

    // Whether an annotation was made before.
    private static native boolean annotationsMade0();
}
