import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.reflect.Field;
import java.lang.reflect.Method;

// An annotation that fields have and no method does, looked up on a method as gson's
// ReflectiveTypeAdapterFactory looks `@SerializedName` up on a record's accessor: the lookup
// finds nothing for every method.
public class AnnotationsNoMethodHas {
    @Retention(RetentionPolicy.RUNTIME)
    @interface Named {
        String value();
    }

    static class Point {
        @Named("x-coordinate")
        public int x;

        public int y;

        public int x() {
            return x;
        }
    }

    public static void main(String[] args) throws Exception {
        Point p = new Point();
        System.out.println(p.x + p.y);
        for (Field field : new Field[] {Point.class.getField("x"), Point.class.getField("y")}) {
            Named named = field.getAnnotation(Named.class);
            System.out.println(field.getName() + " " + (named == null ? "-" : named.value()));
        }
        Method accessor = Point.class.getMethod("x");
        System.out.println("accessor " + (accessor.getAnnotation(Named.class) != null));
        Method none = null;
        try {
            System.out.println(none.getAnnotation(Named.class) != null);
        } catch (NullPointerException e) {
            System.out.println("null accessor");
        }
    }
}
