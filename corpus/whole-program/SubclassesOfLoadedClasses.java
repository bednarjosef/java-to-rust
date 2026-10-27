// `Class.forName(name).asSubclass(type)` passed to a generic method, as log4j's
// `LoaderUtil.newCheckedInstanceOf` creates the classes its properties name: the call's type is a
// capture of `asSubclass`'s `Class<? extends U>`, which javac writes with `U` though it stands for
// the caller's `T` there.
import java.lang.reflect.InvocationTargetException;

class Greeter implements Runnable {
    public Greeter() {
    }

    public void run() {
        System.out.println("hello from Greeter");
    }
}

public class SubclassesOfLoadedClasses {
    static <T> T newInstanceOf(Class<T> clazz) throws InstantiationException, IllegalAccessException,
            InvocationTargetException, NoSuchMethodException {
        return clazz.getConstructor().newInstance();
    }

    static <T> T newCheckedInstanceOf(String className, Class<T> clazz) throws ClassNotFoundException,
            InvocationTargetException, InstantiationException, IllegalAccessException, NoSuchMethodException {
        return newInstanceOf(Class.forName(className).asSubclass(clazz));
    }

    public static void main(String[] args) throws Exception {
        Runnable r = newCheckedInstanceOf("Greeter", Runnable.class);
        r.run();
        try {
            newCheckedInstanceOf("Greeter", CharSequence.class);
        } catch (ClassCastException e) {
            System.out.println("not a CharSequence: " + e.getMessage());
        }
        Class<? extends Runnable> c = Class.forName("Greeter").asSubclass(Runnable.class);
        System.out.println(c.getName());
    }
}
