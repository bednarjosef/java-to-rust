package javax.annotation.processing;

/**
 * The translator's patch of the JDK's {@code AbstractProcessor} (see docs/jdk-overlays.md): the
 * annotations of the processor's class in {@code getSupportedOptions},
 * {@code getSupportedAnnotationTypes} and {@code getSupportedSourceVersion} are read through the
 * translator's {@code TranslatedProcessorAnnotations}, as the translation has no annotation
 * objects.
 */
class AbstractProcessor {
    @Edit(find = "SupportedOptions so = this.getClass().getAnnotation(SupportedOptions.class);",
            replace = "String[] so = TranslatedProcessorAnnotations.values0(this.getClass(), \"javax.annotation.processing.SupportedOptions\");")
    @Edit(find = "arrayToSet(so.value(),", replace = "arrayToSet(so,")
    public Set<String> getSupportedOptions();

    @Edit(find = "SupportedAnnotationTypes sat = this.getClass().getAnnotation(SupportedAnnotationTypes.class);",
            replace = "String[] sat = TranslatedProcessorAnnotations.values0(this.getClass(), \"javax.annotation.processing.SupportedAnnotationTypes\");")
    @Edit(find = "arrayToSet(sat.value(),", replace = "arrayToSet(sat,")
    public Set<String> getSupportedAnnotationTypes();

    @Edit(find = "SupportedSourceVersion ssv = this.getClass().getAnnotation(SupportedSourceVersion.class);",
            replace = "String[] ssv = TranslatedProcessorAnnotations.values0(this.getClass(), \"javax.annotation.processing.SupportedSourceVersion\");")
    // Not set to null first: the translation has no null enums.
    @Edit(find = "SourceVersion sv = null;", replace = "SourceVersion sv;")
    @Edit(find = "sv = ssv.value();", replace = "sv = SourceVersion.valueOf(ssv[0]);")
    public SourceVersion getSupportedSourceVersion();
}
