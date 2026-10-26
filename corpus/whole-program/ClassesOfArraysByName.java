// `Class.forName` of an array's descriptor, as netty asks for `[Ljava.lang.foreign.MemoryLayout;`:
// the array's class, which does not initialize its element class, or `ClassNotFoundException` for
// an array of a class that does not exist. The class found is used at once, as a receiver.
public class ClassesOfArraysByName {
    static class Layout {
        static {
            System.out.println("Layout initialized");
        }
    }

    static String missing() {
        try {
            return Class.forName("[Lno.such.Missing;").getName();
        } catch (ClassNotFoundException e) {
            return "no such class";
        }
    }

    public static void main(String[] args) throws ClassNotFoundException {
        System.out.println(missing());
        Class<?> layouts = Class.forName("[[LClassesOfArraysByName$Layout;");
        System.out.println(layouts.getName() + " " + layouts.getSimpleName());
        System.out.println(layouts.getComponentType().getComponentType() == Layout.class);
        System.out.println(Class.forName("ClassesOfArraysByName$Layout").getSimpleName());
    }
}
