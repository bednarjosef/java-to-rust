package java.lang.reflect;

/**
 * The translator's patch of the JDK's {@code GenericDeclaration} (see docs/jdk-overlays.md): not
 * an {@code AnnotatedElement}, so that a class can be one, as {@code TypeVariable}'s
 * {@code getGenericDeclaration()} gives it, without the annotations of every element.
 */
@Edit(find = "extends AnnotatedElement", replace = "")
class GenericDeclaration {
}
