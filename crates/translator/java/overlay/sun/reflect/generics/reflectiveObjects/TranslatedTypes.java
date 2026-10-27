package sun.reflect.generics.reflectiveObjects;

import java.lang.reflect.Type;

/**
 * The generic types that reflection answers ({@code Class.getGenericSuperclass()},
 * {@code Field.getGenericType()}), which the
 * translator writes from the types javac gives, as the JDK's parser makes them from a class
 * file's {@code Signature} attribute: the JDK's own classes, so they print, compare and hash as
 * in Java (see {@code lower/generic_types.rs}).
 */
final class TranslatedTypes {
    private TranslatedTypes() {}

    /** {@code raw<arguments>}, a member of {@code owner} (null for a top-level or local class). */
    static Type parameterized(Class<?> raw, Type[] arguments, Type owner) {
        return ParameterizedTypeImpl.make(raw, arguments, owner);
    }

    /** A wildcard: {@code ?}, {@code ? extends upper} or {@code ? super lower}. */
    static Type wildcard(Type[] upper, Type[] lower) {
        return WildcardTypeImpl.make(upper, lower);
    }

    /** {@code component[]}, of a component that is not a class. */
    static Type array(Type component) {
        return GenericArrayTypeImpl.make(component);
    }

    /**
     * The type variable {@code name}, which is not supported yet: its {@code TypeVariable} would
     * need the class or method that declares it as a {@code GenericDeclaration}.
     */
    static Type typeVariable(String name) {
        throw new UnsupportedOperationException("the type variable " + name
                + " as a java.lang.reflect.Type, not supported yet");
    }

    /** A type that the translator does not make, which {@code why} says. */
    static Type unsupported(String why) {
        throw new UnsupportedOperationException(why + " not supported yet");
    }
}
