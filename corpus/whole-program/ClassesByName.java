// Objects created from a class name, as the JDK's locale providers create their adapters:
// `(T) Class.forName(name).getDeclaredConstructor().newInstance()`. The translation knows every
// class of the program, so it creates an object of the class with that name; a name of no class
// of the program fails loudly. The reflection exception in the catch, which translated code
// cannot throw, is left out of it.
import java.lang.reflect.InvocationTargetException;

abstract class Greeter {
    abstract String greet(String who);
}

class English extends Greeter {
    String greet(String who) {
        return "hello, " + who;
    }
}

class Czech extends Greeter {
    String greet(String who) {
        return "ahoj, " + who;
    }
}

class Counter {
    static int created;

    Counter() {
        created++;
    }
}

public class ClassesByName {
    static final String[] NAMES = {"English", "Czech"};

    static Greeter greeter(String name) {
        try {
            return (Greeter) Class.forName(name).getDeclaredConstructor().newInstance();
        } catch (NoSuchMethodException | InvocationTargetException | ClassNotFoundException
                 | IllegalAccessException | InstantiationException e) {
            throw new IllegalStateException("no greeter " + name, e);
        }
    }

    @SuppressWarnings("deprecation")
    static Counter counter() throws ReflectiveOperationException {
        return (Counter) (Class.forName("Counter").newInstance());
    }

    public static void main(String[] args) throws Exception {
        for (String name : NAMES) {
            System.out.println(greeter(name).greet("world"));
        }
        counter();
        counter();
        System.out.println("counters: " + Counter.created);
    }
}
