import java.lang.annotation.Annotation;
import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;
import java.lang.reflect.Method;

// Annotation types declared in classes, as netty's `ChannelHandlerMask.Skip` is, which it tests
// each handler's methods for: their classes are named by their binary names (`Outer$Skip`).
public class NestedAnnotationTypes {
    @Target(ElementType.METHOD)
    @Retention(RetentionPolicy.RUNTIME)
    @interface Skip {}

    static class Holder {
        @Retention(RetentionPolicy.RUNTIME)
        @interface Label {
            String value() default "none";
        }
    }

    public static class Handler {
        @Skip
        public void read(Object message) {}

        public void write(Object message) {}
    }

    public static class Named {
        @Holder.Label("named")
        public void run() {}

        @Holder.Label
        public void stop() {}
    }

    static boolean skippable(Class<?> type, String name) throws Exception {
        Method m = type.getMethod(name, Object.class);
        return m.isAnnotationPresent(Skip.class);
    }

    public static void main(String[] args) throws Exception {
        System.out.println(skippable(Handler.class, "read") + " " + skippable(Handler.class, "write"));
        Class<?> skip = Skip.class;
        System.out.println(skip.getName() + " " + skip.getSimpleName() + " " + skip.getCanonicalName());
        System.out.println(skip.isAnnotation() + " " + skip.isInterface());
        for (String name : new String[] {"run", "stop"}) {
            Holder.Label label = Named.class.getMethod(name).getAnnotation(Holder.Label.class);
            System.out.println(name + " " + label.value() + " " + label.annotationType().getName());
            System.out.println(label);
        }
        for (Annotation a : Handler.class.getMethod("read", Object.class).getDeclaredAnnotations()) {
            System.out.println(a.annotationType().getSimpleName() + " " + a + " " + (a instanceof Skip));
        }
    }
}
