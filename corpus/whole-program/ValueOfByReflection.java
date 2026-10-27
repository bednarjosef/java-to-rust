import java.lang.reflect.Method;
import java.lang.reflect.Modifier;

// jopt-simple's converters, which Minecraft's server parses its options with: the class of an
// option's values (Integer.class) is passed down, its static valueOf(String) found by name and
// checked, and called through Method.invoke.
public class ValueOfByReflection {
    static Object convert(Class<?> type, String raw) throws Exception {
        Method valueOf = type.getMethod("valueOf", String.class);
        int modifiers = valueOf.getModifiers();
        System.out.println(Modifier.isPublic(modifiers) + " " + Modifier.isStatic(modifiers) + " "
                + (valueOf.getReturnType() == type));
        return valueOf.invoke(null, raw);
    }

    public static void main(String[] args) throws Exception {
        System.out.println(convert(Integer.class, "25565"));
        System.out.println(convert(Long.class, "-7"));
        System.out.println(convert(Boolean.class, "true"));
        try {
            convert(Integer.class, "port");
        } catch (java.lang.reflect.InvocationTargetException e) {
            System.out.println(e.getCause());
        }
    }
}
