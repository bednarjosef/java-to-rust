import java.lang.annotation.Annotation;
import java.lang.annotation.Documented;
import java.lang.annotation.Retention;

// Class literals of annotation types used as values, as javac's Check asks a symbol for
// `getAnnotation(Documented.class)`: the class is an annotation type and an interface, and what a
// generic method returns for it is an `Annotation`.
public class AnnotationClassValues {
    static <A extends Annotation> A get(Class<A> type, String[] present) {
        if (!type.isAnnotation()) {
            throw new IllegalArgumentException("Not an annotation type: " + type);
        }
        for (String name : present) {
            if (name.equals(type.getName())) {
                System.out.println("found " + type.getSimpleName());
            }
        }
        return null;
    }

    static String describe(Class<?> c) {
        return c + " " + c.getName() + " " + c.isAnnotation() + " " + c.isInterface();
    }

    public static void main(String[] args) {
        String[] present = {"java.lang.annotation.Documented"};
        System.out.println(get(Documented.class, present) != null);
        System.out.println(get(Retention.class, present) == null);
        System.out.println(describe(Documented.class));
        Class<?> r = Retention.class;
        System.out.println(Documented.class == r);
        System.out.println(Documented.class.equals(Documented.class));
        try {
            get((Class) String.class, present);
        } catch (IllegalArgumentException e) {
            System.out.println(e.getMessage());
        }
    }
}
