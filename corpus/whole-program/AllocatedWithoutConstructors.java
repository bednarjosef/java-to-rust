import java.io.ObjectInputStream;
import java.io.ObjectStreamClass;
import java.lang.reflect.Field;
import java.lang.reflect.Method;

// The ways gson looks for to make an object without running its constructor: `allocateInstance`
// of `sun.misc.Unsafe`, which refuses a class that has no objects, and the methods of
// `ObjectStreamClass` and `ObjectInputStream` that Android has and HotSpot does not, whose
// lookups throw `NoSuchMethodException`.
public class AllocatedWithoutConstructors {
    interface Shape {
        double area();
    }

    abstract static class Base {
        abstract int size();
    }

    static String lookUp(Class<?> c, String name, Class<?>... types) {
        try {
            Method m = c.getDeclaredMethod(name, types);
            m.setAccessible(true);
            return "found " + m.getName();
        } catch (Exception e) {
            return e.getClass().getName();
        }
    }

    static void allocate(sun.misc.Unsafe unsafe, Class<?> c) {
        try {
            Object o = unsafe.allocateInstance(c);
            System.out.println("made " + o.getClass().getName());
        } catch (InstantiationException e) {
            System.out.println("InstantiationException: " + e.getMessage());
        }
    }

    public static void main(String[] args) throws Exception {
        System.out.println(ObjectStreamClass.class.getName() + " " + ObjectInputStream.class.getSimpleName());
        System.out.println(lookUp(ObjectStreamClass.class, "getConstructorId", Class.class));
        System.out.println(lookUp(ObjectStreamClass.class, "newInstance", Class.class, int.class));
        System.out.println(lookUp(ObjectInputStream.class, "newInstance", Class.class, Class.class));
        try {
            ObjectStreamClass.class.getDeclaredField("constructorId");
        } catch (NoSuchFieldException e) {
            System.out.println("no field");
        }

        Class<?> unsafeClass = Class.forName("sun.misc.Unsafe");
        Field f = unsafeClass.getDeclaredField("theUnsafe");
        f.setAccessible(true);
        sun.misc.Unsafe unsafe = (sun.misc.Unsafe) f.get(null);
        allocate(unsafe, Shape.class);
        allocate(unsafe, Base.class);
        allocate(unsafe, Number.class);
        allocate(unsafe, Runnable.class);
        allocate(unsafe, int[].class);
    }
}
