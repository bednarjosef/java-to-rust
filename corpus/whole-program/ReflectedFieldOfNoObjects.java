import java.lang.reflect.Field;

// A field that may hold null, of an abstract class the program never creates objects of, which
// reflection reads (as netty's `AbstractInternalLogger.name` where no logger is made): the read
// never runs, and yet has the field's type.
public class ReflectedFieldOfNoObjects {
    abstract static class Named {
        String name;
    }

    static class Child extends Named {}

    static String nameOf(Named named) {
        return named == null ? "nobody" : named.name;
    }

    static void rename(Named named, String name) {
        named.name = name;
    }

    public static void main(String[] args) throws Exception {
        Object o = args.length > 5 ? new Object() : "x";
        System.out.println(o instanceof Child child ? nameOf(child) : nameOf(null));
        try {
            rename(null, args.length > 5 ? "x" : null);
        } catch (NullPointerException e) {
            System.out.println("NullPointerException");
        }
        Field name = Named.class.getDeclaredField(args.length > 5 ? "other" : "name");
        try {
            System.out.println(name.get(null));
        } catch (NullPointerException e) {
            System.out.println("NullPointerException");
        }
    }
}
