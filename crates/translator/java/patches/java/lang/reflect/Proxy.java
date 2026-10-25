package java.lang.reflect;

import java.lang.invoke.MethodHandle;
import java.lang.invoke.MethodHandles;

/**
 * The translator's patch of the JDK's {@code Proxy} (see docs/jdk-overlays.md), the superclass of
 * the proxy classes that the translator writes (see {@code program/reach/proxies.rs}): defining
 * and finding them is the translator's {@code TranslatedProxies}, so what the JDK does to generate
 * a class at run time and cache it is left out. A default method of an interface, which
 * {@code InvocationHandler.invokeDefault} calls through a method handle of the proxy class, fails
 * loudly.
 */
class Proxy {
    @Remove
    private static final ClassLoaderValue<Constructor<?>> proxyCache;

    @Remove
    private static Constructor<?> getProxyConstructor(ClassLoader loader, Class<?>... interfaces);

    @Remove
    private static final class ProxyBuilder {}

    @Remove
    private static Object newProxyInstance(Constructor<?> cons, InvocationHandler h);

    @Remove
    private static final ClassValue<ConcurrentHashMap<Method, MethodHandle>> DEFAULT_METHODS_MAP;

    @Remove
    private static ConcurrentHashMap<Method, MethodHandle> defaultMethodMap(Class<?> proxyClass);

    @Remove
    static MethodHandle defaultMethodHandle(Class<? extends Proxy> proxyClass, Method method);

    @Remove
    private static Class<?> findProxyInterfaceOrElseThrow(Class<?> proxyClass, Method method);

    @Remove
    private static MethodHandles.Lookup proxyClassLookup(MethodHandles.Lookup caller, Class<?> proxyClass);

    @Replace
    @Deprecated
    public static Class<?> getProxyClass(ClassLoader loader, Class<?>... interfaces) throws IllegalArgumentException {
        return TranslatedProxies.getProxyClass(loader, interfaces);
    }

    @Replace
    public static Object newProxyInstance(ClassLoader loader, Class<?>[] interfaces, InvocationHandler h) {
        return TranslatedProxies.newProxyInstance(loader, interfaces, h);
    }

    @Replace
    public static boolean isProxyClass(Class<?> cl) {
        return TranslatedProxies.isProxyClass(cl);
    }

    @Replace
    public static InvocationHandler getInvocationHandler(Object proxy) throws IllegalArgumentException {
        return TranslatedProxies.getInvocationHandler(proxy);
    }

    @Replace
    static Object invokeDefault(Object proxy, Method method, Object[] args, Class<?> caller) throws Throwable {
        throw new UnsupportedOperationException(
                "a default method called through InvocationHandler.invokeDefault, which the translation does not do");
    }
}
