package sun.reflect.generics.reflectiveObjects;

import java.lang.reflect.Type;

/**
 * The translator's patch of the JDK's {@code TypeVariableImpl} (see docs/jdk-overlays.md): a
 * translation makes a class's type variables from the declarations javac gives (see
 * {@code TranslatedTypes}), where the JDK parses them from the class's signature. Its bounds are
 * made when they are first asked for, as the JDK's are, by {@code TranslatedTypes.bounds}: a
 * bound may name the variable itself ({@code E extends Enum<E>}). Only a class declares one: a
 * method's type variables are not made. A translated type variable is not an
 * {@code AnnotatedElement} (see {@code java/lang/reflect/TypeVariable.java}), so its annotation
 * methods go.
 */
@Edit(find = "extends LazyReflectiveObjectGenerator implements", replace = "implements")
@Edit(find = "private volatile Object[] bounds;", replace = "private volatile Type[] bounds;")
class TypeVariableImpl {
    @Remove
    private TypeVariableImpl(D decl, String n, FieldTypeSignature[] bs, GenericsFactory f);

    @Remove
    public static <T extends GenericDeclaration> TypeVariableImpl<T> make(T decl, String name,
            FieldTypeSignature[] bs, GenericsFactory f);

    @Add
    TypeVariableImpl(D decl, String n) {
        genericDeclaration = decl;
        name = n;
    }

    @Replace
    public Type[] getBounds() {
        Type[] value = bounds;
        if (value == null) {
            value = TranslatedTypes.bounds((Class<?>) genericDeclaration, name);
            bounds = value;
        }
        return value.clone();
    }

    @Replace
    public D getGenericDeclaration() {
        return genericDeclaration;
    }

    @Remove
    public <T extends Annotation> T getAnnotation(Class<T> annotationClass);

    @Remove
    public <T extends Annotation> T getDeclaredAnnotation(Class<T> annotationClass);

    @Remove
    public <T extends Annotation> T[] getAnnotationsByType(Class<T> annotationClass);

    @Remove
    public <T extends Annotation> T[] getDeclaredAnnotationsByType(Class<T> annotationClass);

    @Remove
    public Annotation[] getAnnotations();

    @Remove
    public Annotation[] getDeclaredAnnotations();

    @Remove
    public AnnotatedType[] getAnnotatedBounds();

    @Remove
    private int typeVarIndex();

    @Remove
    private static Map<Class<? extends Annotation>, Annotation> mapAnnotations(Annotation[] annos);
}
