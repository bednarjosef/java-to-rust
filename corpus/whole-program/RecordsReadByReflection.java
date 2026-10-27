import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.lang.reflect.ParameterizedType;
import java.lang.reflect.RecordComponent;
import java.lang.reflect.Type;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

// Records that a JSON reader makes by reflection, as gson makes a service's responses: the class
// passed in goes into a field of a token, out of reach of the callers of a parameter; whether it
// is a record, its components and its canonical constructor come from `Class`'s own methods found
// by reflection (as gson finds them, to run on old JDKs too); and the records inside it are found
// only through the generic types of its fields, without a class literal of their own.
public class RecordsReadByReflection {
    record Endpoint(String uri, List<String> validUris) {
        Endpoint(String uri) {
            this(uri, List.of());
        }
    }

    record Endpoints(Map<String, Endpoint> endpoints) {}

    record Response(String product, Endpoints session, int version) {}

    record Plain(String name, long size) {}

    static class NotARecord {
        String name;
    }

    // gson's `TypeToken`.
    static final class Token<T> {
        final Type type;
        final Class<? super T> raw;

        @SuppressWarnings("unchecked")
        Token(Type type) {
            this.type = type;
            this.raw = (Class<? super T>) rawType(type);
        }
    }

    static Class<?> rawType(Type type) {
        if (type instanceof Class<?> c) {
            return c;
        }
        if (type instanceof ParameterizedType p) {
            return (Class<?>) p.getRawType();
        }
        throw new IllegalArgumentException("no class of " + type);
    }

    interface RecordHelper {
        boolean isRecord(Class<?> raw);

        String[] componentNames(Class<?> raw) throws ReflectiveOperationException;

        <T> Constructor<T> canonical(Class<T> raw) throws ReflectiveOperationException;
    }

    // gson's `RecordSupportedHelper`.
    static final class ReflectiveHelper implements RecordHelper {
        private final Method isRecord;
        private final Method getRecordComponents;
        private final Method getName;
        private final Method getType;

        ReflectiveHelper() throws NoSuchMethodException, ClassNotFoundException {
            isRecord = Class.class.getMethod("isRecord");
            getRecordComponents = Class.class.getMethod("getRecordComponents");
            Class<?> component = Class.forName("java.lang.reflect.RecordComponent");
            getName = component.getMethod("getName");
            getType = component.getMethod("getType");
        }

        public boolean isRecord(Class<?> raw) {
            try {
                return (boolean) isRecord.invoke(raw);
            } catch (ReflectiveOperationException e) {
                throw new RuntimeException(e);
            }
        }

        public String[] componentNames(Class<?> raw) throws ReflectiveOperationException {
            Object[] components = (Object[]) getRecordComponents.invoke(raw);
            String[] names = new String[components.length];
            for (int i = 0; i < components.length; i++) {
                names[i] = (String) getName.invoke(components[i]);
            }
            return names;
        }

        public <T> Constructor<T> canonical(Class<T> raw) throws ReflectiveOperationException {
            Object[] components = (Object[]) getRecordComponents.invoke(raw);
            Class<?>[] types = new Class<?>[components.length];
            for (int i = 0; i < components.length; i++) {
                types[i] = (Class<?>) getType.invoke(components[i]);
            }
            return raw.getDeclaredConstructor(types);
        }
    }

    // The helper of a JDK without records.
    static final class NoRecords implements RecordHelper {
        public boolean isRecord(Class<?> raw) {
            return false;
        }

        public String[] componentNames(Class<?> raw) {
            throw new UnsupportedOperationException();
        }

        public <T> Constructor<T> canonical(Class<T> raw) {
            throw new UnsupportedOperationException();
        }
    }

    static RecordHelper helper() {
        try {
            return new ReflectiveHelper();
        } catch (NoSuchMethodException | ClassNotFoundException e) {
            return new NoRecords();
        }
    }

    static final RecordHelper HELPER = helper();

    // A value of the JSON-like tree `json` (maps, lists, strings and numbers) as `type`.
    static Object read(Object json, Type type) throws ReflectiveOperationException {
        Class<?> raw = rawType(type);
        if (raw == String.class || raw == int.class || raw == Integer.class) {
            return json;
        }
        if (raw == List.class) {
            Type element = ((ParameterizedType) type).getActualTypeArguments()[0];
            List<Object> out = new ArrayList<>();
            for (Object o : (List<?>) json) {
                out.add(read(o, element));
            }
            return out;
        }
        if (raw == Map.class) {
            Type value = ((ParameterizedType) type).getActualTypeArguments()[1];
            Map<String, Object> out = new LinkedHashMap<>();
            for (Map.Entry<?, ?> e : ((Map<?, ?>) json).entrySet()) {
                out.put((String) e.getKey(), read(e.getValue(), value));
            }
            return out;
        }
        if (HELPER.isRecord(raw)) {
            Map<?, ?> object = (Map<?, ?>) json;
            Constructor<?> constructor = HELPER.canonical(raw);
            String[] names = HELPER.componentNames(raw);
            Object[] values = new Object[names.length];
            for (int i = 0; i < names.length; i++) {
                Field field = raw.getDeclaredField(names[i]);
                values[i] = read(object.get(names[i]), field.getGenericType());
            }
            return constructor.newInstance(values);
        }
        throw new IllegalArgumentException("cannot read a " + raw.getSimpleName());
    }

    @SuppressWarnings("unchecked")
    static <T> T fromJson(Object json, Class<T> c) throws ReflectiveOperationException {
        Token<T> token = new Token<>(c);
        return (T) read(json, token.type);
    }

    // The record `r` as JSON, through the accessors of its components.
    static String write(Object value) throws ReflectiveOperationException {
        if (value instanceof Record r) {
            StringBuilder out = new StringBuilder("{");
            for (RecordComponent component : r.getClass().getRecordComponents()) {
                if (out.length() > 1) {
                    out.append(", ");
                }
                Object part = component.getAccessor().invoke(r);
                out.append('"').append(component.getName()).append("\": ").append(write(part));
            }
            return out.append('}').toString();
        }
        if (value instanceof Map<?, ?> map) {
            StringBuilder out = new StringBuilder("{");
            for (Map.Entry<?, ?> e : map.entrySet()) {
                if (out.length() > 1) {
                    out.append(", ");
                }
                out.append('"').append(e.getKey()).append("\": ").append(write(e.getValue()));
            }
            return out.append('}').toString();
        }
        if (value instanceof List<?> list) {
            List<String> parts = new ArrayList<>();
            for (Object o : list) {
                parts.add(write(o));
            }
            return "[" + String.join(", ", parts) + "]";
        }
        return value instanceof String s ? '"' + s + '"' : String.valueOf(value);
    }

    public static void main(String[] args) throws Exception {
        Map<String, Object> join = new LinkedHashMap<>();
        join.put("uri", "https://example.com/join");
        join.put("validUris", List.of("https://example.com/a", "https://example.com/b"));
        Map<String, Object> endpoints = new LinkedHashMap<>();
        endpoints.put("join", join);
        Map<String, Object> session = new LinkedHashMap<>();
        session.put("endpoints", endpoints);
        Map<String, Object> json = new LinkedHashMap<>();
        json.put("product", "minecraft");
        json.put("session", session);
        json.put("version", 3);

        Response response = fromJson(json, Response.class);
        System.out.println(response);
        System.out.println(response.session().endpoints().get("join").validUris().size());
        System.out.println(write(response));
        try {
            fromJson(Map.of(), NotARecord.class);
        } catch (IllegalArgumentException e) {
            System.out.println(e.getMessage());
        }

        System.out.println(Plain.class.isRecord() + " " + NotARecord.class.isRecord() + " " + String.class.isRecord());
        System.out.println(Arrays.toString(Plain.class.getRecordComponents()));
        System.out.println(NotARecord.class.getRecordComponents() == null);
        for (RecordComponent c : Plain.class.getRecordComponents()) {
            System.out.println(c.getName() + " " + c.getType().getName() + " " + c.getGenericType()
                    + " " + c.getDeclaringRecord().getSimpleName() + " " + c.getAccessor().getName());
        }
        Constructor<?>[] constructors = Endpoint.class.getDeclaredConstructors();
        int[] counts = new int[constructors.length];
        for (int i = 0; i < constructors.length; i++) {
            counts[i] = constructors[i].getParameterCount();
        }
        Arrays.sort(counts);
        System.out.println(Arrays.toString(counts));
        System.out.println(write(new Plain("disk", 1L << 40)));
    }
}
