package sun.reflect.annotation;

import java.lang.annotation.Annotation;
import java.util.Map;

/**
 * The translator's patch of the JDK's {@code AnnotationParser} (see docs/jdk-overlays.md). The
 * JDK's makes an annotation as a {@code java.lang.reflect.Proxy} of the annotation type whose
 * handler is an {@code AnnotationInvocationHandler}; a translated program has no proxies, and
 * the handler, patched, is the annotation itself.
 */
class AnnotationParser {
    @Replace
    public static Annotation annotationForMap(final Class<? extends Annotation> type,
                                              final Map<String, Object> memberValues)
    {
        return new AnnotationInvocationHandler(type, memberValues);
    }
}
