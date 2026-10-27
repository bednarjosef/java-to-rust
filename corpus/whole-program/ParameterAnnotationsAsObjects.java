// `getParameterAnnotations()` where the program also holds an `Annotation[][]` as an
// `Object[]`, which makes every `Annotation[][]` one (a covariant array): each parameter's array
// is stored as an `Object`. Otherwise as `ParameterAnnotations`.
import java.lang.annotation.Annotation;
import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;
import java.lang.reflect.Constructor;
import java.lang.reflect.Method;

@Retention(RetentionPolicy.RUNTIME)
@Target(ElementType.PARAMETER)
@interface Attribute {
    String value();
    String defaultValue() default "";
}

@Retention(RetentionPolicy.RUNTIME)
@Target(ElementType.PARAMETER)
@interface Element {
    String value();
}

@Retention(RetentionPolicy.RUNTIME)
@Target(ElementType.METHOD)
@interface Factory {}

public class ParameterAnnotationsAsObjects {
    static class Appender {
        final String name;

        Appender(@Attribute("name") String name, int unannotated) {
            this.name = name;
        }

        @Factory
        static Appender create(
                @Attribute(value = "name", defaultValue = "Console") String name,
                @SuppressWarnings("unused") int size,
                @Element("Layout") @Attribute("layout") Object layout) {
            return new Appender(name, size);
        }

        static Appender plain(String name, int size) {
            return new Appender(name, size);
        }
    }

    static void show(String what, Annotation[][] all) {
        StringBuilder out = new StringBuilder(what + ": " + all.length);
        for (Annotation[] annotations : all) {
            out.append(" [");
            for (Annotation a : annotations) {
                if (a instanceof Attribute attribute) {
                    out.append(" attribute ").append(attribute.value()).append('=').append(attribute.defaultValue());
                } else if (a instanceof Element element) {
                    out.append(" element ").append(element.value());
                }
            }
            out.append(" ]");
        }
        System.out.println(out);
    }

    public static void main(String[] args) throws Exception {
        Object[] held = Appender.class.getDeclaredMethod("plain", String.class, int.class).getParameterAnnotations();
        System.out.println(held.length);
        for (Method m : new Method[] {
            Appender.class.getDeclaredMethod("create", String.class, int.class, Object.class),
            Appender.class.getDeclaredMethod("plain", String.class, int.class),
        }) {
            show(m.getName() + (m.isAnnotationPresent(Factory.class) ? " (factory)" : ""), m.getParameterAnnotations());
        }
        Constructor<Appender> c = Appender.class.getDeclaredConstructor(String.class, int.class);
        show("constructor", c.getParameterAnnotations());
    }
}
