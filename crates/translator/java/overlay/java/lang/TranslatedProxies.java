package java.lang;

import java.lang.reflect.InvocationHandler;

/**
 * The translator's {@code Proxy.isProxyClass} and {@code Proxy.getInvocationHandler}. A
 * translated program makes no proxies ({@code Proxy.newProxyInstance} is rejected), so no class
 * is a proxy class, except that Java makes annotations as proxies, which the translation makes
 * as the patched {@code AnnotationInvocationHandler} itself (see
 * {@code lower/annotation_objects.rs}). Code that finds a proxy class goes on to treat it as one
 * (its interfaces, its handler), which an annotation's class is not, so asking about it fails
 * loudly.
 */
final class TranslatedProxies {
    private static final String ANNOTATION = "sun.reflect.annotation.AnnotationInvocationHandler";

    private TranslatedProxies() {}

    static boolean isProxyClass(Class<?> cl) {
        // `Proxy.class.isAssignableFrom(cl)`, which throws without a message.
        if (cl == null) {
            throw new NullPointerException();
        }
        if (cl.getName().equals(ANNOTATION)) {
            throw new UnsupportedOperationException(
                    "the class of an annotation, which is not a proxy class in the translation");
        }
        return false;
    }

    static InvocationHandler getInvocationHandler(Object proxy) {
        // False, or for an annotation it throws.
        isProxyClass(proxy.getClass());
        throw new IllegalArgumentException("not a proxy instance");
    }
}
