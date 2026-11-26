import java.lang.reflect.Array;
import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.GenericArrayType;
import java.lang.reflect.Modifier;
import java.lang.reflect.ParameterizedType;
import java.lang.reflect.Type;
import java.lang.reflect.WildcardType;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

// Plain objects that a JSON reader makes by reflection, as gson makes them: the class passed in
// goes into a field of a token, through gson's way to the class of a type, which makes an array
// for an array type, so that the points-to facts cannot tell which classes the lookups get; each
// object is made with its constructor without parameters, found by `getDeclaredConstructor()`,
// so its field initializers run, and then its fields are set from the values read; and the
// classes of the objects inside it are found only through the generic types of its fields,
// without a class literal of their own.
public class ObjectsReadByReflection {
    static class Item {
        String name;
        int count;
    }

    static class Order {
        String customer;
        List<Item> items = new ArrayList<>();
        Map<String, Integer> tags;
        double total;
        transient String note = "unset";

        Order() {}

        Order(String customer) {
            this.customer = customer;
        }
    }

    // The class of a type, as gson's `GsonTypes.getRawType` finds it: that of an array type is
    // the class of an array made of its component's class, whose class the reflection table has
    // no members of.
    static Class<?> rawType(Type type) {
        if (type instanceof Class<?> c) {
            return c;
        }
        if (type instanceof ParameterizedType p) {
            return (Class<?>) p.getRawType();
        }
        if (type instanceof GenericArrayType g) {
            return Array.newInstance(rawType(g.getGenericComponentType()), 0).getClass();
        }
        if (type instanceof WildcardType w) {
            return rawType(w.getUpperBounds()[0]);
        }
        return Object.class;
    }

    // What gson's `TypeToken` holds: the type and its class.
    static final class Token {
        final Type type;
        final Class<?> raw;

        Token(Type type) {
            this.type = type;
            this.raw = rawType(type);
        }
    }

    // The value of a type, from a value read: a map is an object of the type's class, a list a
    // list of its type argument's values.
    static Object read(Token token, Object value) throws Exception {
        if (value instanceof List<?> list) {
            Type element = ((ParameterizedType) token.type).getActualTypeArguments()[0];
            List<Object> out = new ArrayList<>();
            for (Object v : list) {
                out.add(read(new Token(element), v));
            }
            return out;
        }
        if (value instanceof Map<?, ?> map && token.raw != Map.class) {
            Constructor<?> constructor = token.raw.getDeclaredConstructor();
            constructor.setAccessible(true);
            Object object = constructor.newInstance();
            for (Field f : token.raw.getDeclaredFields()) {
                if (Modifier.isTransient(f.getModifiers()) || !map.containsKey(f.getName())) {
                    continue;
                }
                f.setAccessible(true);
                Object v = map.get(f.getName());
                if (f.getType() == int.class) {
                    f.setInt(object, ((Number) v).intValue());
                } else if (f.getType() == double.class) {
                    f.setDouble(object, ((Number) v).doubleValue());
                } else {
                    f.set(object, read(new Token(f.getGenericType()), v));
                }
            }
            return object;
        }
        return value;
    }

    // An object's fields as text, read by reflection.
    static String write(Object object) throws Exception {
        if (object instanceof List<?> list) {
            List<String> out = new ArrayList<>();
            for (Object v : list) {
                out.add(write(v));
            }
            return out.toString();
        }
        if (object == null || object instanceof String || object instanceof Number || object instanceof Map) {
            return String.valueOf(object);
        }
        StringBuilder out = new StringBuilder("{");
        for (Field f : object.getClass().getDeclaredFields()) {
            if (!Modifier.isTransient(f.getModifiers())) {
                f.setAccessible(true);
                out.append(out.length() > 1 ? ", " : "").append(f.getName()).append('=').append(write(f.get(object)));
            }
        }
        return out.append('}').toString();
    }

    static <T> T fromMap(Map<String, Object> map, Class<T> type) throws Exception {
        return type.cast(read(new Token(type), map));
    }

    public static void main(String[] args) throws Exception {
        Map<String, Object> tea = new LinkedHashMap<>();
        tea.put("name", "tea");
        tea.put("count", 2);
        Map<String, Object> cake = new LinkedHashMap<>();
        cake.put("name", "cake");
        cake.put("count", 1);
        Map<String, Object> json = new LinkedHashMap<>();
        json.put("customer", "Ada");
        json.put("items", List.of(tea, cake));
        json.put("tags", Map.of("a", 1));
        json.put("total", 7.5);
        Order order = fromMap(json, Order.class);
        System.out.println(order.customer + " ordered " + order.items.size() + " items, first "
                + order.items.get(0).name + " x" + order.items.get(0).count + ", note " + order.note);
        System.out.println(write(order));
        Order other = new Order("Bob");
        other.total = 3;
        System.out.println(write(other));
    }
}
