import java.lang.annotation.Annotation;
import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;
import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.Method;

@Retention(RetentionPolicy.RUNTIME)
@interface Label {
    String value();

    int order() default 0;
}

@Retention(RetentionPolicy.RUNTIME)
@interface Version {
    double value();
}

@Retention(RetentionPolicy.RUNTIME)
@Target(ElementType.TYPE_USE)
@interface Checked {
}

@interface Dropped {
}

@Retention(RetentionPolicy.RUNTIME)
@interface Range {
    Label low();

    Label high() default @Label(value = "top", order = 9);
}

// `getAnnotation` and `getDeclaredAnnotations` of a method, a field and a constructor, as gson
// and the JDK's beans read them, and the identity of the annotations: Java gives the same object
// on every call for the same declaration, also through a subclass that inherits the member.
public class MemberAnnotations {
    static class Base {
        @Label(value = "id", order = 1)
        @Version(2.5)
        @Dropped
        public int id;

        @Label("name")
        public String name;

        public long plain;

        @Range(low = @Label(value = "bottom", order = 2))
        public int span;

        @Range(low = @Label("floor"), high = @Label("ceiling"))
        public int reach;

        @Label("make")
        public Base() {
        }

        @Version(1.0)
        public Base(int id) {
            this.id = id;
        }

        @Label(value = "describe", order = 3)
        public @Checked String describe() {
            return "base " + id;
        }

        public void quiet() {
        }
    }

    static class Derived extends Base {
        @Label("derived")
        public Derived() {
        }

        @Version(3.0)
        public void extra() {
        }
    }

    abstract static class Shape {
        @Label("shape")
        public Shape() {
        }
    }

    static String show(Annotation[] annotations) {
        StringBuilder out = new StringBuilder("[");
        for (Annotation a : annotations) {
            out.append(out.length() == 1 ? "" : ", ").append(a);
        }
        return out.append("]").toString();
    }

    public static void main(String[] args) throws Exception {
        for (String name : new String[] {"id", "name", "plain"}) {
            Field f = Base.class.getField(name);
            Label label = f.getAnnotation(Label.class);
            Version version = f.getAnnotation(Version.class);
            System.out.println(name + ": " + (label == null ? "no label" : label.value() + " " + label.order())
                    + ", " + (version == null ? "no version" : String.valueOf(version.value()))
                    + ", dropped " + f.getAnnotation(Dropped.class) + ", present " + f.isAnnotationPresent(Label.class));
            System.out.println("  " + show(f.getDeclaredAnnotations()) + " " + f.getAnnotations().length);
        }
        Field id = Base.class.getField("id");
        System.out.println(id.getAnnotation(Label.class) == id.getAnnotation(Label.class));
        System.out.println(id.getAnnotation(Label.class) == Derived.class.getField("id").getAnnotation(Label.class));
        System.out.println(id.getDeclaredAnnotations()[0] == id.getAnnotation(Label.class));
        System.out.println(id.getDeclaredAnnotations() == id.getDeclaredAnnotations());

        for (String name : new String[] {"describe", "quiet", "extra"}) {
            Method m = Derived.class.getMethod(name);
            Label label = m.getAnnotation(Label.class);
            System.out.println(name + ": " + label + " " + show(m.getDeclaredAnnotations())
                    + " checked " + m.getAnnotation(Checked.class));
        }
        Method describe = Base.class.getMethod("describe");
        System.out.println(describe.getAnnotation(Label.class) == Derived.class.getMethod("describe").getAnnotation(Label.class));
        System.out.println(describe.getAnnotation(Label.class).equals(Base.class.getField("id").getAnnotation(Label.class)));

        Constructor<?> made = Base.class.getConstructor();
        System.out.println(made.getAnnotation(Label.class) + " " + made.getAnnotation(Version.class));
        System.out.println(Base.class.getConstructor(int.class).getAnnotation(Version.class));
        System.out.println(show(Derived.class.getConstructor().getAnnotations()));
        System.out.println(Shape.class.getConstructor().getAnnotation(Label.class));
        System.out.println(made.getAnnotation(Label.class) == Base.class.getConstructor().getAnnotation(Label.class));
        try {
            Shape.class.getConstructor().newInstance();
        } catch (InstantiationException e) {
            System.out.println("abstract");
        }
        Range span = Base.class.getField("span").getAnnotation(Range.class);
        Range reach = Base.class.getField("reach").getAnnotation(Range.class);
        System.out.println(span + " " + span.low().value() + " " + span.high().order());
        System.out.println(reach + " " + reach.high().value());
        System.out.println((span.low() == span.low()) + " " + (span.high() == Derived.class.getField("span")
                .getAnnotation(Range.class).high()) + " " + span.low().equals(reach.low()));
        Method none = null;
        try {
            none.getAnnotation(Label.class);
        } catch (NullPointerException e) {
            System.out.println("null method");
        }
    }
}
