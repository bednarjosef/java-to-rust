package sun.reflect.generics.reflectiveObjects;

import java.lang.reflect.Type;
import java.lang.reflect.TypeVariable;

/**
 * The generic types that reflection answers ({@code Class.getGenericSuperclass()},
 * {@code getGenericInterfaces()}, {@code getTypeParameters()}, {@code Field.getGenericType()}
 * and a type variable's bounds), which the translator writes from the types javac gives, as the
 * JDK's parser makes them from a class file's {@code Signature} attribute: the JDK's own classes,
 * so they print, compare and hash as in Java (see {@code lower/generic_types.rs}). The natives
 * are written by the lowering, each a chain of tests of the class against the classes of the
 * program that declare what it asks for.
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

    /** The type variable {@code name} that the class {@code declaration} declares. */
    static TypeVariable<?> typeVariable(Class<?> declaration, String name) {
        return new TypeVariableImpl<>(declaration, name);
    }

    /** {@code c.getTypeParameters()}: the type variables that the class declares, in order. */
    static TypeVariable<?>[] typeParameters(Class<?> c) {
        TypeVariable<?>[] declared = typeParameters0(c);
        return declared != null ? declared : new TypeVariable<?>[0];
    }

    /** {@code c.getGenericInterfaces()}: the interfaces the class implements, with their type arguments. */
    static Type[] genericInterfaces(Class<?> c) {
        Type[] generic = genericInterfaces0(c);
        if (generic != null) {
            return generic;
        }
        Class<?>[] interfaces = c.getInterfaces();
        Type[] types = new Type[interfaces.length];
        for (int i = 0; i < interfaces.length; i++) {
            types[i] = interfaces[i];
        }
        return types;
    }

    /**
     * The bounds of the type variable {@code name} that the class {@code declaration} declares:
     * {@code Object} where it has none.
     */
    static Type[] bounds(Class<?> declaration, String name) {
        Type[] declared = bounds0(declaration, name);
        return declared != null ? declared : new Type[] {Object.class};
    }

    /** The type variables of a class that declares any, null for any other. */
    private static native TypeVariable<?>[] typeParameters0(Class<?> c);

    /** The interfaces of a class that implements one with type arguments, null for any other. */
    private static native Type[] genericInterfaces0(Class<?> c);

    /** The bounds of a type variable that has any, null for any other. */
    private static native Type[] bounds0(Class<?> declaration, String name);

    /** A type that the translator does not make, which {@code why} says. */
    static Type unsupported(String why) {
        throw new UnsupportedOperationException(why + " not supported yet");
    }
}
