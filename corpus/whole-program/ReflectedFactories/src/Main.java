import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Factories found by reflection as log4j finds its plugins': a converter's static
 * `newInstance(String[])`, checked by its parameters' classes, and an appender's generic static
 * `newBuilder()`, whose generic builder's fields are set by name.
 */
public class Main {
    static boolean validParameters(Class<?>[] types) {
        for (Class<?> type : types) {
            if (!type.isAssignableFrom(factories.Configuration.class)
                    && !(type.isArray() && "[Ljava.lang.String;".equals(type.getName()))) {
                return false;
            }
        }
        return true;
    }

    static String describe(Method method) {
        StringBuilder text = new StringBuilder(method.getName()).append('(');
        for (Class<?> type : method.getParameterTypes()) {
            text.append(type.getName()).append(' ');
        }
        return text.append(") ").append(method.getReturnType().getName()).toString();
    }

    @SuppressWarnings("unchecked")
    public static void main(String[] args) throws Exception {
        String converterName = args.length > 5 ? args[0] : "factories.Upper";
        String appenderName = args.length > 5 ? args[1] : "factories.Appender";
        Map<String, Class<factories.Converter>> rules = new LinkedHashMap<>();
        rules.put("upper", (Class<factories.Converter>) Class.forName(converterName));
        Class<factories.Converter> converterClass = rules.get("upper");
        List<String> lines = new ArrayList<>();
        Method newInstance = null;
        for (Method method : converterClass.getDeclaredMethods()) {
            lines.add("converter " + describe(method));
            if (Modifier.isStatic(method.getModifiers())
                    && method.getDeclaringClass().equals(converterClass)
                    && method.getName().equals("newInstance")
                    && validParameters(method.getParameterTypes())) {
                newInstance = method;
            }
        }
        Class<?> appenderClass = Class.forName(appenderName);
        for (Method method : appenderClass.getDeclaredMethods()) {
            lines.add("appender " + describe(method));
        }
        Collections.sort(lines);
        lines.forEach(System.out::println);

        factories.Converter converter = (factories.Converter) newInstance.invoke(null, (Object) new String[] {"a", "b"});
        System.out.println(converter.convert("hi"));

        Object builder = appenderClass.getDeclaredMethod("newBuilder").invoke(null);
        Field name = factories.Appender.Builder.class.getDeclaredField("name");
        name.setAccessible(true);
        name.set(builder, "console");
        Method build = factories.Appender.Builder.class.getMethod("build");
        System.out.println(((factories.Appender) build.invoke(builder)).name());
        Method first = appenderClass.getMethod("first", Object[].class);
        System.out.println(first.invoke(null, (Object) new String[] {"one", "two"}));
    }
}
