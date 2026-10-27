package java.lang.reflect;

/**
 * The translator's patch of the JDK's {@code TypeVariable} (see docs/jdk-overlays.md): not an
 * {@code AnnotatedElement}, as the annotations of a type variable and of its bounds are not in the
 * translation.
 */
@Edit(find = "extends Type, AnnotatedElement", replace = "extends Type")
class TypeVariable {
    @Remove
    AnnotatedType[] getAnnotatedBounds();
}
