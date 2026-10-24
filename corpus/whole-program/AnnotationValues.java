import java.lang.annotation.Annotation;
import java.lang.annotation.Inherited;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;

@Retention(RetentionPolicy.RUNTIME)
@Inherited
@interface Since {
    double value();
}

@Retention(RetentionPolicy.RUNTIME)
@interface Named {
    String value();

    Class<?>[] alternate() default {};

    int[] codes() default {1, 2};

    int weight() default 3;

    long size() default 7;

    char mark() default 'x';

    Class<?> adapter() default Object.class;
}

@interface Dropped {
    String value();
}

// `getAnnotation` of a class: the values the declaration gives and the defaults of the rest,
// `null` where the class does not have the annotation, and an `@Inherited` one from the
// superclass, as gson's `Excluder` reads `@Since`; the annotation's `toString`, and its identity.
public class AnnotationValues {
    @Since(1.5)
    @Named(value = "base", alternate = {String.class, Integer.class}, weight = 10, codes = 4)
    @Dropped("gone")
    static class Base {}

    static class Derived extends Base {}

    @Named(value = "solo", adapter = Derived.class, size = 40)
    static class Solo {}

    @Named(value = "one", alternate = Long.class, codes = {})
    static class Single {}

    static class Plain {}

    static String names(Class<?>[] classes) {
        StringBuilder out = new StringBuilder();
        for (Class<?> c : classes) {
            out.append(out.length() == 0 ? "" : "/").append(c.getSimpleName());
        }
        return out.toString();
    }

    static void show(Class<?> c) {
        Since since = c.getAnnotation(Since.class);
        Named named = c.getAnnotation(Named.class);
        System.out.println(c.getSimpleName() + ": since " + (since == null ? "none" : String.valueOf(since.value()))
                + ", dropped " + c.getAnnotation(Dropped.class));
        if (named != null) {
            Class<?>[] alternate = named.alternate();
            int[] codes = named.codes();
            System.out.println("  named " + named.value() + " " + names(alternate) + " " + alternate.length + " "
                    + codes.length + " " + named.weight() + " " + named.size() + " " + named.mark() + " "
                    + named.adapter().getSimpleName());
            if (alternate.length > 0) {
                alternate[0] = Plain.class;
            }
            if (codes.length > 0) {
                codes[0] = 99;
            }
            System.out.println("  again " + names(named.alternate()) + " " + (codes.length > 0 ? named.codes()[0] : -1));
            Annotation annotation = named;
            System.out.println("  type " + annotation.annotationType().getSimpleName() + ", same "
                    + named.equals(c.getAnnotation(Named.class)) + ", hash "
                    + (named.hashCode() == c.getAnnotation(Named.class).hashCode()));
            System.out.println("  " + named);
        }
    }

    public static void main(String[] args) {
        Class<?>[] classes = {Base.class, Derived.class, Solo.class, Single.class, Plain.class, String.class};
        for (Class<?> c : classes) {
            show(c);
        }
        System.out.println(Base.class.getAnnotation(Since.class).equals(Derived.class.getAnnotation(Since.class)));
        System.out.println(Base.class.getAnnotation(Named.class).equals(Solo.class.getAnnotation(Named.class)));
        // The same object on every call, also through a subclass that inherits it.
        System.out.println(Base.class.getAnnotation(Named.class) == Base.class.getAnnotation(Named.class));
        System.out.println(Base.class.getAnnotation(Since.class) == Derived.class.getAnnotation(Since.class));
    }
}
