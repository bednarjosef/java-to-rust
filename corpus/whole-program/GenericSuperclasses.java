import java.lang.reflect.GenericArrayType;
import java.lang.reflect.ParameterizedType;
import java.lang.reflect.Type;
import java.lang.reflect.WildcardType;
import java.util.Arrays;
import java.util.List;
import java.util.Map;
import java.util.Objects;

// `getGenericSuperclass()`, mostly through a type token as gson writes it (an anonymous subclass
// of a generic class reads its own type argument): parameterized types, wildcards, generic arrays,
// classes, member types and their owners, compared with a `ParameterizedType` of the program's
// own, as gson's `$Gson$Types` makes them.
public class GenericSuperclasses {
    static class TypeToken<T> {
        final Type type;
        final Class<?> rawType;

        protected TypeToken() {
            Type superclass = getClass().getGenericSuperclass();
            if (superclass instanceof ParameterizedType parameterized) {
                if (parameterized.getRawType() == TypeToken.class) {
                    type = parameterized.getActualTypeArguments()[0];
                    rawType = rawType(type);
                    return;
                }
            } else if (superclass == TypeToken.class) {
                throw new IllegalStateException("TypeToken must be created with a type argument");
            }
            throw new IllegalStateException("not a direct subclass: " + superclass);
        }

        static Class<?> rawType(Type type) {
            if (type instanceof Class<?> c) {
                return c;
            } else if (type instanceof ParameterizedType p) {
                return (Class<?>) p.getRawType();
            } else if (type instanceof GenericArrayType) {
                return Object[].class;
            } else if (type instanceof WildcardType w) {
                return rawType(w.getUpperBounds()[0]);
            }
            throw new IllegalArgumentException("unexpected type " + type);
        }

        @Override
        public String toString() {
            return type.getTypeName();
        }
    }

    /** A parameterized type of the program's own, as gson's. */
    static final class OwnParameterized implements ParameterizedType {
        private final Type owner;
        private final Type raw;
        private final Type[] args;

        OwnParameterized(Type owner, Type raw, Type... args) {
            this.owner = owner;
            this.raw = raw;
            this.args = args.clone();
        }

        public Type[] getActualTypeArguments() {
            return args.clone();
        }

        public Type getRawType() {
            return raw;
        }

        public Type getOwnerType() {
            return owner;
        }

        @Override
        public boolean equals(Object other) {
            return other instanceof ParameterizedType that
                    && Objects.equals(owner, that.getOwnerType())
                    && raw.equals(that.getRawType())
                    && Arrays.equals(args, that.getActualTypeArguments());
        }

        @Override
        public int hashCode() {
            return Arrays.hashCode(args) ^ raw.hashCode() ^ Objects.hashCode(owner);
        }
    }

    static class Outer<A> {
        class Inner<B> {}

        class Plain {}

        static class Nested<C> {}
    }

    static class Base<K, V> {}

    static class Holder extends Base<String, List<Integer>> {}

    static class Simple extends Base<int[], Object[][]> {}

    static class NotGeneric {}

    static class Child extends NotGeneric {}

    enum Color {
        RED,
        GREEN
    }

    static void show(String label, TypeToken<?> token) {
        Type type = token.type;
        String kind = type instanceof Class ? "class"
                : type instanceof ParameterizedType ? "parameterized"
                : type instanceof GenericArrayType ? "array"
                : type instanceof WildcardType ? "wildcard" : "other";
        System.out.println(label + ": " + type + " | " + type.getTypeName() + " | " + kind + " | "
                + token.rawType.getName());
    }

    static void describe(Type type) {
        if (type instanceof ParameterizedType p) {
            System.out.println("  raw " + p.getRawType() + ", owner " + p.getOwnerType() + ", "
                    + p.getActualTypeArguments().length + " arguments");
            for (Type a : p.getActualTypeArguments()) {
                describe(a);
            }
        } else if (type instanceof WildcardType w) {
            System.out.println("  wildcard " + Arrays.toString(w.getUpperBounds()) + " "
                    + Arrays.toString(w.getLowerBounds()));
        } else if (type instanceof GenericArrayType g) {
            System.out.println("  array of " + g.getGenericComponentType());
            describe(g.getGenericComponentType());
        } else {
            System.out.println("  " + type);
        }
    }

    public static void main(String[] args) {
        TypeToken<List<String>> strings = new TypeToken<List<String>>() {};
        show("list", strings);
        show("map", new TypeToken<Map<String, List<Integer>>>() {});
        show("extends", new TypeToken<List<? extends Number>>() {});
        show("super", new TypeToken<List<? super Integer>>() {});
        show("any", new TypeToken<List<?>>() {});
        show("array", new TypeToken<List<String>[]>() {});
        show("class", new TypeToken<String>() {});
        show("ints", new TypeToken<int[]>() {});
        show("strings", new TypeToken<String[][]>() {});
        show("nested", new TypeToken<Outer.Nested<String>>() {});
        show("inner", new TypeToken<Outer<String>.Inner<Integer>>() {});
        show("plain", new TypeToken<Outer<Long>.Plain>() {});
        show("entry", new TypeToken<Map.Entry<String, Integer>>() {});
        show("bounded", new TypeToken<Map<? extends Comparable<String>, ? super List<?>[]>>() {});
        for (TypeToken<?> token : List.of(new TypeToken<Map.Entry<String, int[]>>() {},
                new TypeToken<Outer<String>.Inner<Integer>>() {}, new TypeToken<List<? super Integer>[][]>() {})) {
            describe(token.type);
        }

        // Equal types are equal, with equal hash codes, whoever made them.
        Type again = new TypeToken<List<String>>() {}.type;
        Type own = new OwnParameterized(null, List.class, String.class);
        System.out.println(strings.type.equals(again) + " " + (strings.type.hashCode() == again.hashCode()) + " "
                + (strings.type == again));
        System.out.println(strings.type.equals(own) + " " + own.equals(strings.type) + " "
                + (strings.type.hashCode() == own.hashCode()));
        System.out.println(strings.type.equals(new TypeToken<List<Integer>>() {}.type) + " "
                + strings.type.equals(List.class) + " " + strings.type.equals(null));
        Type inner = new TypeToken<Outer<String>.Inner<Integer>>() {}.type;
        Type ownInner = new OwnParameterized(new OwnParameterized(GenericSuperclasses.class, Outer.class, String.class),
                Outer.Inner.class, Integer.class);
        System.out.println(inner.equals(ownInner) + " " + (inner.hashCode() == ownInner.hashCode()));
        Type wildcard = ((ParameterizedType) new TypeToken<List<?>>() {}.type).getActualTypeArguments()[0];
        Type wildcard2 = ((ParameterizedType) new TypeToken<Map<?, ?>>() {}.type).getActualTypeArguments()[1];
        System.out.println(wildcard.equals(wildcard2) + " " + (wildcard.hashCode() == wildcard2.hashCode()));
        Type generic = new TypeToken<List<String>[]>() {}.type;
        System.out.println(generic.equals(new TypeToken<List<String>[]>() {}.type) + " "
                + ((GenericArrayType) generic).getGenericComponentType().equals(strings.type));
        System.out.println(strings.type.getClass().getName() + " " + wildcard.getClass().getName() + " "
                + generic.getClass().getName());

        // Named classes, through class literals and objects.
        System.out.println(Holder.class.getGenericSuperclass());
        System.out.println(new Holder().getClass().getGenericSuperclass().getTypeName());
        System.out.println(Simple.class.getGenericSuperclass());
        describe(Simple.class.getGenericSuperclass());
        System.out.println(Child.class.getGenericSuperclass() + " " + NotGeneric.class.getGenericSuperclass());
        System.out.println(Object.class.getGenericSuperclass() + " " + Runnable.class.getGenericSuperclass() + " "
                + int.class.getGenericSuperclass() + " " + String[].class.getGenericSuperclass());
        System.out.println(Color.class.getGenericSuperclass() + " " + Color.RED.getClass().getGenericSuperclass());
        Class<?>[] classes = {Holder.class, Child.class, Outer.Nested.class, Color.class, Integer.class};
        for (Class<?> c : classes) {
            System.out.println(c.getSimpleName() + " " + c.getGenericSuperclass());
        }

        try {
            new TypeToken() {};
        } catch (IllegalStateException e) {
            System.out.println(e.getMessage());
        }
    }
}
