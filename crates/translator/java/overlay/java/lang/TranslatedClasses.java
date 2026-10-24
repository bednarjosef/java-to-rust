package java.lang;

/**
 * The translator's static methods of {@code Class} that are Java code over class literals, which
 * the translator's classes are (see {@code program::REDIRECTED_CALLS}), as {@code Class} is the
 * runtime's and not translated.
 */
final class TranslatedClasses {
    private TranslatedClasses() {}

    /** {@code Class.forPrimitiveName(name)}: the class of the primitive type named, or null. */
    static Class<?> forPrimitiveName(String primitiveName) {
        return switch (primitiveName) {
            case "boolean" -> boolean.class;
            case "byte" -> byte.class;
            case "char" -> char.class;
            case "short" -> short.class;
            case "int" -> int.class;
            case "long" -> long.class;
            case "float" -> float.class;
            case "double" -> double.class;
            case "void" -> void.class;
            default -> null;
        };
    }
}
