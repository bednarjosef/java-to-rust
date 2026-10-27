import java.lang.reflect.GenericDeclaration;
import java.lang.reflect.ParameterizedType;
import java.lang.reflect.Type;
import java.lang.reflect.TypeVariable;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collection;
import java.util.List;
import java.util.Map;

// A class's type variables (`getTypeParameters()`, their bounds and the class that declares them)
// and the interfaces it implements with type arguments (`getGenericInterfaces()`), walked as gson
// walks them to find the element type of a collection: from a field's `List<String>` up through
// the JDK's interfaces to `Collection<E>`, then each `E` resolved back to `String`.
public class GenericTypeVariables {
    static class Holder {
        List<String> names;
        Map<String, Integer> counts;
        Strings strings;
        Collection<Long> longs;
    }

    static class Strings extends ArrayList<String> {}

    static class Box<T extends Comparable<T>> implements Comparable<Box<T>> {
        T value;

        @Override
        public int compareTo(Box<T> other) {
            return value.compareTo(other.value);
        }
    }

    static class Pair<A, B extends Number & Comparable<B>> {}

    interface Source<T> extends Iterable<T>, Comparable<Source<T>> {}

    /** gson's `getGenericSupertype`: the supertype `supertype` of `context`, with its arguments. */
    static Type genericSupertype(Type context, Class<?> rawType, Class<?> supertype) {
        if (supertype == rawType) {
            return context;
        }
        if (supertype.isInterface()) {
            Class<?>[] interfaces = rawType.getInterfaces();
            for (int i = 0; i < interfaces.length; i++) {
                if (interfaces[i] == supertype) {
                    return rawType.getGenericInterfaces()[i];
                } else if (supertype.isAssignableFrom(interfaces[i])) {
                    return genericSupertype(rawType.getGenericInterfaces()[i], interfaces[i], supertype);
                }
            }
        }
        if (!rawType.isInterface()) {
            while (rawType != Object.class) {
                Class<?> rawSupertype = rawType.getSuperclass();
                if (rawSupertype == supertype) {
                    return rawType.getGenericSuperclass();
                } else if (supertype.isAssignableFrom(rawSupertype)) {
                    return genericSupertype(rawType.getGenericSuperclass(), rawSupertype, supertype);
                }
                rawType = rawSupertype;
            }
        }
        return supertype;
    }

    /** gson's `resolve` for a type variable: the argument that `context` gives it, as often as it is one. */
    static Type resolve(Type context, Class<?> contextRawType, Type toResolve) {
        while (toResolve instanceof TypeVariable<?> variable) {
            GenericDeclaration declaration = variable.getGenericDeclaration();
            if (!(declaration instanceof Class<?> declaredBy)) {
                return variable;
            }
            Type declaredByType = genericSupertype(context, contextRawType, declaredBy);
            if (!(declaredByType instanceof ParameterizedType parameterized)) {
                return variable;
            }
            int index = Arrays.asList(declaredBy.getTypeParameters()).indexOf(variable);
            toResolve = parameterized.getActualTypeArguments()[index];
        }
        return toResolve;
    }

    /** gson's `equals` of two type variables. */
    static boolean sameVariable(Type a, Type b) {
        if (!(a instanceof TypeVariable) || !(b instanceof TypeVariable)) {
            return false;
        }
        TypeVariable<?> va = (TypeVariable<?>) a;
        TypeVariable<?> vb = (TypeVariable<?>) b;
        return va.getGenericDeclaration() == vb.getGenericDeclaration() && va.getName().equals(vb.getName());
    }

    static Class<?> rawType(Type type) {
        return type instanceof ParameterizedType p ? (Class<?>) p.getRawType() : (Class<?>) type;
    }

    /** gson's `getCollectionElementType`. */
    static Type elementType(Type context) {
        Class<?> raw = rawType(context);
        Type collection = genericSupertype(context, raw, Collection.class);
        if (collection instanceof ParameterizedType p) {
            return resolve(context, raw, p.getActualTypeArguments()[0]);
        }
        return Object.class;
    }

    static void describe(Class<?> c) {
        System.out.println(c.getSimpleName() + " interfaces " + Arrays.toString(c.getGenericInterfaces()));
        for (TypeVariable<?> variable : c.getTypeParameters()) {
            System.out.println("  " + variable + " of " + variable.getGenericDeclaration()
                    + " bounds " + Arrays.toString(variable.getBounds()));
        }
    }

    public static void main(String[] args) throws Exception {
        for (String name : new String[] {"names", "counts", "strings", "longs"}) {
            Type type = Holder.class.getDeclaredField(name).getGenericType();
            Type element = Collection.class.isAssignableFrom(rawType(type)) ? elementType(type) : null;
            System.out.println(name + ": " + type.getTypeName() + " elements " + element);
        }
        describe(Box.class);
        describe(Pair.class);
        describe(Source.class);
        describe(Strings.class);
        describe(Holder.class);
        TypeVariable<?> first = Box.class.getTypeParameters()[0];
        TypeVariable<?> again = Box.class.getTypeParameters()[0];
        TypeVariable<?> other = Source.class.getTypeParameters()[0];
        System.out.println(first.equals(again) + " " + (first.hashCode() == again.hashCode()) + " "
                + first.equals(other) + " " + first.getName() + " " + first.getTypeName());
        System.out.println(sameVariable(first, again) + " " + sameVariable(first, other) + " "
                + Box.class.getTypeParameters().length + " " + Holder.class.getTypeParameters().length);
        Type bound = first.getBounds()[0];
        Type argument = ((ParameterizedType) bound).getActualTypeArguments()[0];
        System.out.println(bound + " names " + argument + " " + argument.equals(first));
        for (Class<?> c : new Class<?>[] {Comparable.class, Enum.class, Class.class}) {
            for (TypeVariable<?> variable : c.getTypeParameters()) {
                System.out.println(
                        c.getSimpleName() + " " + variable + " bounds " + Arrays.toString(variable.getBounds()));
            }
        }
        Type comparable = Box.class.getGenericInterfaces()[0];
        System.out.println(resolve(comparable, Comparable.class, Comparable.class.getTypeParameters()[0]));
    }
}
