package sun.reflect.annotation;

import java.lang.annotation.Annotation;
import java.lang.reflect.Array;
import java.util.Map;

/**
 * The translator's patch of the JDK's {@code AnnotationInvocationHandler} (see
 * docs/jdk-overlays.md): the handler is the annotation itself, as a translated program has no
 * proxies ({@code AnnotationParser.annotationForMap}), and no longer an {@code InvocationHandler},
 * which nothing calls. Its {@code annotationType},
 * {@code toString}, {@code hashCode} and {@code equals} are the handler's answers to the proxy's
 * methods; its class is not the annotation type's, so {@code Class.cast} and
 * {@code Class.isInstance} of an annotation type test the type it holds
 * ({@code lower/annotation_objects.rs}). An annotation type is an interface that extends
 * {@code Annotation} alone, which the JDK's constructor checks by reflection; the patched one
 * checks that it is an annotation type. {@code equals} compares the values of the type's members,
 * which {@code TranslatedAnnotations.members0} names, as the JDK's does over the type's methods.
 * An annotation type is not translated, so a call of one of its members is a call of
 * {@code member}, which answers it as the proxy's {@code invoke} does.
 */
@Edit(find = "class AnnotationInvocationHandler implements InvocationHandler, Serializable {",
      replace = "class AnnotationInvocationHandler implements Serializable, Annotation {")
class AnnotationInvocationHandler {
    @Edit(from = "Class<?>[] superInterfaces", to = "type.getName());",
          replace = "if (!type.isAnnotation()) throw new AnnotationFormatError(\"Attempt to create proxy for a non-annotation type: \" + type.getName());")
    AnnotationInvocationHandler(Class<? extends Annotation> type, Map<String, Object> memberValues);

    @Add
    public Class<? extends Annotation> annotationType() {
        return type;
    }

    @Add
    Object member(String name) {
        Object result = memberValues.get(name);
        if (result.getClass().isArray() && Array.getLength(result) != 0)
            result = cloneArray(result);
        return result;
    }

    @Add
    public String toString() {
        return toStringImpl();
    }

    @Add
    public int hashCode() {
        return hashCodeImpl();
    }

    @Add
    public boolean equals(Object o) {
        if (o == this)
            return true;
        if (!(o instanceof AnnotationInvocationHandler other) || other.type != type)
            return false;
        for (String member : TranslatedAnnotations.members0(type)) {
            if (!memberValueEquals(memberValues.get(member), other.memberValues.get(member)))
                return false;
        }
        return true;
    }
}
