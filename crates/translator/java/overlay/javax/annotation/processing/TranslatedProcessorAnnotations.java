package javax.annotation.processing;

/**
 * The annotations that {@code AbstractProcessor} reads of its processor's class, which the
 * translator writes from the program's sources, as the translation has no annotation objects
 * (see {@code lower/processors.rs}).
 */
final class TranslatedProcessorAnnotations {
    private TranslatedProcessorAnnotations() {
    }

    /**
     * The {@code value} of the annotation {@code annotation} (its qualified name, a literal) of
     * the class {@code c} itself, as strings: an enum constant by its name. Null when the class
     * does not have the annotation.
     */
    static native String[] values0(Class<?> c, String annotation);
}
