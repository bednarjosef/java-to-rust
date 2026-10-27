import java.lang.annotation.Annotation;
import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;
import java.lang.reflect.Field;

// Classes named only by the values of annotations, which code that reads the annotations makes
// objects of, as log4j's `PluginVisitors.findVisitor` does with `@PluginVisitorStrategy` and its
// `ConstraintValidators` with `@Constraint`.
public class AnnotationClassValues {
    public interface Visitor {
        String visit(Field field);
    }

    public static class ValueVisitor implements Visitor {
        public ValueVisitor() {}

        public String visit(Field field) {
            return "value " + field.getName();
        }
    }

    public static class RequiredVisitor implements Visitor {
        public RequiredVisitor() {}

        public String visit(Field field) {
            return "required " + field.getName();
        }
    }

    @Retention(RetentionPolicy.RUNTIME)
    @Target(ElementType.ANNOTATION_TYPE)
    public @interface VisitorStrategy {
        Class<? extends Visitor> value();
    }

    @Retention(RetentionPolicy.RUNTIME)
    @Target(ElementType.FIELD)
    @VisitorStrategy(ValueVisitor.class)
    public @interface Value {}

    @Retention(RetentionPolicy.RUNTIME)
    @Target(ElementType.FIELD)
    @VisitorStrategy(RequiredVisitor.class)
    public @interface Required {}

    static class Settings {
        @Value
        String name;

        @Required
        int size;

        String plain;
    }

    static Visitor findVisitor(Class<? extends Annotation> annotation) throws Exception {
        VisitorStrategy strategy = annotation.getAnnotation(VisitorStrategy.class);
        if (strategy == null) {
            return null;
        }
        return strategy.value().getDeclaredConstructor().newInstance();
    }

    public static void main(String[] args) throws Exception {
        for (String name : new String[] {"name", "size", "plain"}) {
            Field field = Settings.class.getDeclaredField(name);
            Annotation[] annotations = field.getDeclaredAnnotations();
            if (annotations.length == 0) {
                System.out.println(name + ": no annotations");
            }
            for (Annotation a : annotations) {
                Visitor visitor = findVisitor(a.annotationType());
                System.out.println(name + ": " + (visitor == null ? "no visitor" : visitor.visit(field)));
            }
        }
    }
}
