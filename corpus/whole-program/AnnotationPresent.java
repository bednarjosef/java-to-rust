import java.lang.annotation.Documented;
import java.lang.annotation.Inherited;
import java.lang.annotation.Repeatable;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;

@Retention(RetentionPolicy.RUNTIME)
@interface Kept {}

@Retention(RetentionPolicy.RUNTIME)
@Inherited
@interface Passed {}

@interface Dropped {}

@Retention(RetentionPolicy.RUNTIME)
@Repeatable(Tags.class)
@interface Tag {
    String value();
}

@Retention(RetentionPolicy.RUNTIME)
@interface Tags {
    Tag[] value();
}

// `isAnnotationPresent`, as javac's ClassSymbol asks an annotation type whether it is
// `@Inherited`: what is kept when the program runs, what a subclass inherits, and what a repeated
// annotation leaves (its container).
public class AnnotationPresent {
    @Kept
    @Passed
    @Dropped
    static class Base {}

    static class Derived extends Base {}

    @Kept
    interface Shape {}

    static class Square implements Shape {}

    @Tag("one")
    static class Once {}

    @Tag("one")
    @Tag("two")
    static class Twice {}

    static void show(String name, Class<?> c) {
        System.out.println(name + ": kept " + c.isAnnotationPresent(Kept.class) + ", passed "
                + c.isAnnotationPresent(Passed.class) + ", dropped " + c.isAnnotationPresent(Dropped.class)
                + ", tag " + c.isAnnotationPresent(Tag.class) + ", tags " + c.isAnnotationPresent(Tags.class));
    }

    public static void main(String[] args) {
        show("Base", Base.class);
        show("Derived", Derived.class);
        show("Shape", Shape.class);
        show("Square", Square.class);
        show("Once", Once.class);
        show("Twice", Twice.class);
        show("String", String.class);
        Class<?>[] types = {Kept.class, Passed.class, Documented.class, Retention.class, Inherited.class};
        for (Class<?> type : types) {
            System.out.println(type.getSimpleName() + ": inherited " + type.isAnnotationPresent(Inherited.class)
                    + ", documented " + type.isAnnotationPresent(Documented.class) + ", target "
                    + type.isAnnotationPresent(Target.class));
        }
    }
}
