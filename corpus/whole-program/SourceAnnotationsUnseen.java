// The annotations that are not kept at run time (`@Serial`, `@Override`, `@SuppressWarnings`), which
// reflection never finds on a member, beside one that is: log4j's `PluginBuilder.injectFields`
// lists a builder's fields' annotations, among them a `serialVersionUID`'s `@Serial`.
import java.io.Serial;
import java.io.Serializable;
import java.lang.annotation.Annotation;
import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;
import java.lang.reflect.Field;
import java.lang.reflect.Method;

@Retention(RetentionPolicy.RUNTIME)
@Target({ElementType.FIELD, ElementType.METHOD})
@interface Attribute {
    String value();
}

public class SourceAnnotationsUnseen {
    static class Builder implements Serializable {
        @Serial
        private static final long serialVersionUID = 1L;

        @Attribute("name")
        String name = "console";

        @SuppressWarnings("unused")
        int size;

        @Override
        @Attribute("text")
        public String toString() {
            return name;
        }
    }

    public static void main(String[] args) throws Exception {
        for (String name : new String[] {"serialVersionUID", "name", "size"}) {
            Field f = Builder.class.getDeclaredField(name);
            Annotation[] found = f.getDeclaredAnnotations();
            System.out.print(name + ":");
            for (Annotation a : found) {
                System.out.print(" " + ((Attribute) a).value());
            }
            System.out.println(" (" + found.length + ")");
        }
        Method m = Builder.class.getMethod("toString");
        System.out.println("toString: " + m.getDeclaredAnnotations().length);
    }
}
