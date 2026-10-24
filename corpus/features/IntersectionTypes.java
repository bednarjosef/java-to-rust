// Intersection types: casts such as `(Base & Named) o`, which check every bound as javac's
// erasure does (the later bounds first, then the first, and the type the place wants last), the
// values of such a type, of its erasure, calling the methods of any bound, and the intersections
// that javac infers for a generic method's result, as the least upper bound of its arguments.
public class IntersectionTypes {
    interface Named {
        String name();
    }

    interface Sized {
        int size();
    }

    static class Base {
        int id() {
            return 7;
        }
    }

    interface Tagged {
        default String tag() {
            return describe((Base & Named) this);
        }
    }

    static class Both extends Base implements Named, Sized, Tagged {
        public String name() {
            return "both";
        }

        public int size() {
            return 3;
        }
    }

    static class OnlyBase extends Base {}

    static class Other extends Base implements Named {
        public String name() {
            return "other";
        }

        int id() {
            return 8;
        }
    }

    static class OnlyNamed implements Named, Tagged {
        public String name() {
            return "named";
        }
    }

    static <T extends Base & Named> String describe(T t) {
        return t.name() + ":" + t.id();
    }

    @SuppressWarnings("unchecked")
    static <S extends Base & Named> S make(Object o) {
        return (S) o;
    }

    static <U> U first(U a, U b) {
        return a;
    }

    static String show(Object o) {
        Base b = (Base & Named) o;
        var both = (Base & Named & Sized) o;
        return ((Base & Named) o).name() + " " + ((Base & Sized) o).size() + " " + b.id() + " " + both.size()
                + " " + describe((Base & Named) o);
    }

    static Named named(Object o) {
        return (Base & Named) o;
    }

    static void attempt(String label, Runnable r) {
        try {
            r.run();
        } catch (ClassCastException e) {
            System.out.println(label + ": " + e.getMessage().substring(0, e.getMessage().indexOf(" (")));
        }
    }

    public static void main(String[] args) {
        Object o = new Both();
        System.out.println(show(o));
        System.out.println(named(o).name());
        Base made = make(o);
        System.out.println(made.id());
        Named assigned;
        assigned = (Base & Named) o;
        System.out.println(assigned.name());
        System.out.println(new Both().tag());
        var either = first(new Other(), new Both());
        System.out.println(either.name() + either.id() + first(new Both(), new Other()).name());
        attempt("tag", () -> System.out.println(new OnlyNamed().tag()));
        attempt("only base", () -> System.out.println(show(new OnlyBase())));
        attempt("only named", () -> System.out.println(show(new OnlyNamed())));
        attempt("string", () -> System.out.println(show("text")));
        attempt("named wanted", () -> System.out.println(named("text").name()));
        attempt("made", () -> System.out.println(make(new OnlyBase()).id()));
    }
}
