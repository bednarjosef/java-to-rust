package sun.reflect.annotation;

/**
 * What the patched {@code AnnotationInvocationHandler} asks of an annotation type, which the
 * translator writes from the program's sources, as an annotation type is not translated (see
 * {@code lower/annotation_objects.rs}).
 */
final class TranslatedAnnotations {
    private TranslatedAnnotations() {
    }

    /**
     * The names of the members (the elements) of the annotation type {@code type}, in the order
     * of its declaration.
     */
    static native String[] members0(Class<?> type);
}
