import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;

// Reflection by name over the program's classes: a class found by a computed or spelled-out
// name, its constructors, methods and fields looked up by name and called, read and written,
// inherited methods that dispatch to an override, the exceptions of a name that is not there, and
// those of the code called, which reflection wraps (but `Class.newInstance` does not).
public class ReflectionTable {
    public static class Greeter {
        public String greeting = "hello";
        private int count;
        public static int created;

        public Greeter() {
            created++;
        }

        public Greeter(String greeting, int count) {
            this.greeting = greeting;
            this.count = count;
            created++;
        }

        public String greet(String name) {
            count++;
            return greeting + ", " + name + " #" + count;
        }

        public static int twice(int x) {
            return 2 * x;
        }

        public final int limit = 3;

        public static int fail(int x) {
            if (x > 0) {
                throw new IllegalStateException("bad " + x);
            }
            return 1 / x;
        }

        private long secret() {
            return 42L;
        }

        public void reset() {
            count = 0;
        }
    }

    public static class LoudGreeter extends Greeter {
        public String greet(String name) {
            return super.greet(name).toUpperCase();
        }
    }

    public static class Broken {
        public Broken() {
            throw new UnsupportedOperationException("not made");
        }
    }

    interface Shape {
        double area();
    }

    public static class Square implements Shape {
        public double side = 2;

        public double area() {
            return side * side;
        }

        public String name() {
            return "square";
        }
    }

    public static void main(String[] args) throws Exception {
        Class<?> c = Class.forName(args.length > 0 ? args[0] : "ReflectionTable$Greeter");
        Object g = c.getConstructor().newInstance();
        Method greet = c.getMethod("greet", String.class);
        Method twice = c.getMethod("twice", int.class);
        System.out.println(greet.invoke(g, "world"));
        System.out.println(greet.invoke(g, "again"));
        System.out.println(greet.getName() + " " + greet.getReturnType().getName() + " "
                + greet.getParameterCount() + " " + greet.getModifiers() + " " + twice.getModifiers());
        System.out.println(greet);
        System.out.println(twice.invoke(null, 21));
        Method secret = c.getDeclaredMethod("secret");
        secret.setAccessible(true);
        System.out.println(secret.invoke(g));
        Constructor<?> two = c.getConstructor(String.class, int.class);
        Object made = two.newInstance("hi", 10);
        System.out.println(greet.invoke(made, "you"));
        Field greeting = c.getField("greeting");
        System.out.println(greeting.get(made));
        greeting.set(made, "hey");
        System.out.println(greet.invoke(made, "there"));
        Field count = c.getDeclaredField("count");
        count.setAccessible(true);
        System.out.println(count.getInt(made) + " " + count.getType());
        count.setInt(made, 100);
        System.out.println(greet.invoke(made, "later"));
        System.out.println(c.getField("created").get(null));
        System.out.println(c.getDeclaredFields().length);

        Class<?> loud = Class.forName("ReflectionTable$LoudGreeter");
        Object l = loud.getDeclaredConstructor().newInstance();
        System.out.println(greet.invoke(l, "dispatch"));
        String which = args.length > 0 ? args[0] : "reset";
        System.out.println(loud.getMethod(which).getDeclaringClass().getSimpleName());
        Shape s = (Shape) Class.forName("ReflectionTable$Square").getDeclaredConstructor().newInstance();
        System.out.println(Shape.class.getMethod("area").invoke(s));
        System.out.println(Square.class.getField("side").getDouble(s));
        System.out.println(Class.forName("ReflectionTable$Square").getMethod("name").invoke(s));

        try {
            Class.forName("ReflectionTable$Missing");
        } catch (ClassNotFoundException e) {
            System.out.println("no class");
        }
        try {
            c.getMethod("absent");
        } catch (NoSuchMethodException e) {
            System.out.println("no method");
        }
        try {
            c.getField("count");
        } catch (NoSuchFieldException e) {
            System.out.println("no public field");
        }
        try {
            greet.invoke(g, 5);
        } catch (IllegalArgumentException e) {
            System.out.println("argument type mismatch");
        }
        try {
            greet.invoke(null, "nobody");
        } catch (NullPointerException e) {
            System.out.println("null receiver");
        }
        try {
            Class.forName("ReflectionTable$Greeter").asSubclass(Shape.class);
        } catch (ClassCastException e) {
            System.out.println("not a shape");
        }
        System.out.println(Class.forName("ReflectionTable$Square").asSubclass(Shape.class).getSimpleName());
        Class<?> shape = Class.forName("ReflectionTable$Shape");
        System.out.println(Class.forName("ReflectionTable$Square").asSubclass(shape).getSimpleName());
        try {
            Class.forName("ReflectionTable$Greeter").asSubclass(shape);
        } catch (ClassCastException e) {
            System.out.println("not a shape: " + e.getMessage());
        }
        try {
            Class.forName("ReflectionTable$Square").asSubclass(null);
        } catch (NullPointerException e) {
            System.out.println("null superclass");
        }

        Method fail = c.getMethod("fail", int.class);
        for (int x : new int[] {3, 0}) {
            try {
                fail.invoke(null, x);
            } catch (InvocationTargetException e) {
                System.out.println("wrapped " + e.getCause() + " " + (e.getTargetException() == e.getCause())
                        + " " + e.getMessage());
            }
        }
        Class<?> broken = Class.forName(args.length > 1 ? args[1] : "ReflectionTable$Broken");
        try {
            broken.getConstructor().newInstance();
        } catch (InvocationTargetException e) {
            System.out.println("constructor wrapped " + e.getCause().getMessage());
        }
        try {
            broken.newInstance();
        } catch (UnsupportedOperationException e) {
            System.out.println("not wrapped " + e.getMessage());
        }
        Field limit = c.getField("limit");
        System.out.println(limit.get(made));
        try {
            limit.set(made, 4);
        } catch (IllegalAccessException e) {
            System.out.println("final field");
        }
    }
}
