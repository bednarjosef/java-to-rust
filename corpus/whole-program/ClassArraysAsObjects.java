import java.lang.reflect.Method;
import java.util.Arrays;

// Arrays of classes held as Object[] (a Class<?>[] passed where an Object[] is taken), with the
// arrays of classes that reflection makes: a method's parameter types, the types that getMethod
// is given, and a class's interfaces.
public class ClassArraysAsObjects implements Runnable, Comparable<ClassArraysAsObjects> {
    public void run() {
    }

    public int compareTo(ClassArraysAsObjects o) {
        return 0;
    }

    public static String greet(String name, Integer times) {
        return name + times;
    }

    static String show(Object[] objects) {
        return objects.length + " " + Arrays.toString(objects);
    }

    public static void main(String[] args) throws Exception {
        Class<?>[] types = {String.class, Integer.class};
        System.out.println(show(types));
        Method m = ClassArraysAsObjects.class.getMethod("greet", String.class, Integer.class);
        System.out.println(m.getName() + " " + show(m.getParameterTypes()));
        System.out.println(show(ClassArraysAsObjects.class.getInterfaces()));
    }
}
