package sun.reflect.generics.reflectiveObjects;

/**
 * The translator's patch of the JDK's {@code ParameterizedTypeImpl} (see docs/jdk-overlays.md):
 * a translation makes these from the types javac gives, not by parsing a class file's
 * {@code Signature} attribute (see {@code TranslatedTypes}). The translator passes the owner type
 * as the JDK's parser finds it, so the constructor does not ask the raw type for its declaring
 * class, and it does not check the arguments against the raw type's type parameters, which javac
 * already did.
 */
class ParameterizedTypeImpl {
    @Edit(find = "(ownerType != null) ? ownerType : rawType.getDeclaringClass();", replace = "ownerType;")
    @Edit(find = "validateConstructorArguments();", replace = "")
    private ParameterizedTypeImpl(Class<?> rawType, Type[] actualTypeArguments, Type ownerType);

    @Remove
    private void validateConstructorArguments();
}
