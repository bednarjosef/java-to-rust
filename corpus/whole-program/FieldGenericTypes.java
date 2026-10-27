import java.lang.reflect.Field;
import java.lang.reflect.GenericArrayType;
import java.lang.reflect.ParameterizedType;
import java.lang.reflect.Type;
import java.lang.reflect.WildcardType;
import java.util.Arrays;
import java.util.List;
import java.util.Map;

// `Field.getGenericType()`, as Gson reads each field of a class it binds: a field's type with
// its type arguments, made as the JDK makes it from the field's signature, so it prints,
// compares and hashes as in Java; a field without them answers its class.
public class FieldGenericTypes {
    static class Response {
        List<String> names;
        Map<String, List<Integer>> scores;
        List<? extends Number> numbers;
        Map.Entry<String, Long> entry;
        List<String>[] pages;
        int count;
        String title;
        int[] sizes;
    }

    record Keys(List<String> keys, Map<String, ? super Integer> limits) {}

    static void describe(Type type, String indent) {
        if (type instanceof ParameterizedType p) {
            System.out.println(indent + "parameterized " + p.getRawType().getTypeName() + " owner "
                    + p.getOwnerType());
            for (Type argument : p.getActualTypeArguments()) {
                describe(argument, indent + "  ");
            }
        } else if (type instanceof WildcardType w) {
            System.out.println(indent + "wildcard " + Arrays.toString(w.getUpperBounds()) + " "
                    + Arrays.toString(w.getLowerBounds()));
        } else if (type instanceof GenericArrayType a) {
            System.out.println(indent + "array of");
            describe(a.getGenericComponentType(), indent + "  ");
        } else {
            System.out.println(indent + "class " + type.getTypeName());
        }
    }

    public static void main(String[] args) throws Exception {
        for (String name : new String[] {"names", "scores", "numbers", "entry", "pages", "count", "title", "sizes"}) {
            Field field = Response.class.getDeclaredField(name);
            Type type = field.getGenericType();
            System.out.println(name + ": " + type.getTypeName() + " | " + type);
            describe(type, "  ");
            Type again = field.getGenericType();
            System.out.println("  equal " + type.equals(again) + " " + (type.hashCode() == again.hashCode()));
        }
        for (Field field : Keys.class.getDeclaredFields()) {
            System.out.println(field.getName() + ": " + field.getGenericType());
        }
        Type names = Response.class.getDeclaredField("names").getGenericType();
        Type keys = Keys.class.getDeclaredField("keys").getGenericType();
        System.out.println(names.equals(keys) + " " + names.equals(List.class));
    }
}
