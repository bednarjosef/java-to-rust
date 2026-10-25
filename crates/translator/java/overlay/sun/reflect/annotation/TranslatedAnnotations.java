package sun.reflect.annotation;

import java.lang.annotation.Annotation;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.Map;

/**
 * What the patched {@code AnnotationInvocationHandler} asks of an annotation type, which the
 * translator writes from the program's sources, as an annotation type is not translated (see
 * {@code lower/annotation_objects.rs}).
 */
final class TranslatedAnnotations {
    /** The annotations made so far, by the name that {@code cached} is given. */
    private static final Map<String, Annotation> made = new HashMap<>();

    private TranslatedAnnotations() {
    }

    /**
     * The names of the members (the elements) of the annotation type {@code type}, in the order
     * of its declaration.
     */
    static native String[] members0(Class<?> type);

    /**
     * Notes that an annotation is made, which Java makes as a proxy, of a proxy class that it
     * may define then and number (see {@code java/lang/reflect/TranslatedProxies.java}).
     */
    static native void made0();

    /**
     * The annotation of type {@code type} that a declaration has, as {@code AnnotationParser}
     * makes it: the defaults of the type's members ({@code defaults}, names and values in turn,
     * in the order of the type's declaration), with the values that the declaration gives
     * ({@code values}, in its order) over them, which decides the order {@code toString} lists
     * them in.
     */
    static Annotation annotation(Class<? extends Annotation> type, Object[] defaults, Object[] values) {
        Map<String, Object> memberDefaults = new HashMap<>(0);
        for (int i = 0; i < defaults.length; i += 2)
            memberDefaults.put((String) defaults[i], defaults[i + 1]);
        Map<String, Object> memberValues = new LinkedHashMap<>(memberDefaults);
        for (int i = 0; i < values.length; i += 2)
            memberValues.put((String) values[i], values[i + 1]);
        return new AnnotationInvocationHandler(type, memberValues);
    }

    /**
     * The annotation that {@code key} names, a declaration's of one type, made as
     * {@code annotation} makes it the first time it is asked for: Java's class or member keeps the
     * annotations it has once made, so each call gives the same object.
     */
    static synchronized Annotation cached(String key, Class<? extends Annotation> type, Object[] defaults,
            Object[] values) {
        Annotation found = made.get(key);
        if (found == null) {
            found = annotation(type, defaults, values);
            made.put(key, found);
        }
        return found;
    }

    /** {@code annotation.name()}, of a member of its type. */
    static Object member(Annotation annotation, String name) {
        return ((AnnotationInvocationHandler) annotation).member(name);
    }
}
