package sun.reflect.generics.reflectiveObjects;

/**
 * The translator's patch of the JDK's {@code WildcardTypeImpl} (see docs/jdk-overlays.md): a
 * translation makes wildcards from the types javac gives (see {@code TranslatedTypes}), with
 * their bounds as types, where the JDK keeps the bounds' signatures and makes their types when
 * they are first asked for, through a factory of its parser.
 */
@Edit(find = "extends LazyReflectiveObjectGenerator", replace = "")
@Edit(find = "private volatile Object[] upperBounds;", replace = "private final Type[] upperBounds;")
@Edit(find = "private volatile Object[] lowerBounds;", replace = "private final Type[] lowerBounds;")
class WildcardTypeImpl {
    @Remove
    private WildcardTypeImpl(FieldTypeSignature[] ubs, FieldTypeSignature[] lbs, GenericsFactory f);

    @Remove
    public static WildcardTypeImpl make(FieldTypeSignature[] ubs, FieldTypeSignature[] lbs, GenericsFactory f);

    @Add
    private WildcardTypeImpl(Type[] ubs, Type[] lbs) {
        upperBounds = ubs;
        lowerBounds = lbs;
    }

    /** A wildcard of these bounds: an upper bound of {@code Object} alone where it has none. */
    @Add
    static WildcardTypeImpl make(Type[] ubs, Type[] lbs) {
        return new WildcardTypeImpl(ubs, lbs);
    }

    @Replace
    public Type[] getUpperBounds() {
        return upperBounds.clone();
    }

    @Replace
    public Type[] getLowerBounds() {
        return lowerBounds.clone();
    }
}
