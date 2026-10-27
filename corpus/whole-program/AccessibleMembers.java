import java.lang.reflect.AccessibleObject;
import java.lang.reflect.Constructor;
import java.lang.reflect.Executable;
import java.lang.reflect.Field;
import java.lang.reflect.Member;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.util.Objects;

// Reflection's members held as their supertypes, `AccessibleObject`, `Executable` and `Member`,
// as log4j's `ReflectionUtil.makeAccessible` and gson's `ReflectionHelper.makeAccessible` hold
// them to make them accessible and to describe them.
public class AccessibleMembers {
    public static class Point {
        public int x = 1;
        private int y = 2;
        private static final String NAME = "point";

        public Point() {
        }

        private Point(int x, int y) {
            this.x = x;
            this.y = y;
        }

        public int sum() {
            return x + y;
        }

        private static String describe(String prefix, int times) {
            return prefix + " x" + times;
        }
    }

    // log4j's `ReflectionUtil`.
    static <T extends AccessibleObject & Member> boolean isAccessible(final T member) {
        Objects.requireNonNull(member, "No member provided");
        return Modifier.isPublic(member.getModifiers())
                && Modifier.isPublic(member.getDeclaringClass().getModifiers());
    }

    @SuppressWarnings("deprecation")
    static <T extends AccessibleObject & Member> void makeAccessible(final T member) {
        if (!isAccessible(member) && !member.isAccessible()) {
            member.setAccessible(true);
        }
    }

    @SuppressWarnings("deprecation")
    static void makeAccessible(final Field field) {
        Objects.requireNonNull(field, "No field provided");
        if ((!isAccessible(field) || Modifier.isFinal(field.getModifiers())) && !field.isAccessible()) {
            field.setAccessible(true);
        }
    }

    // gson's `ReflectionHelper`.
    static void forceAccessible(AccessibleObject object) {
        try {
            object.setAccessible(true);
        } catch (Exception exception) {
            throw new IllegalStateException("Failed making " + describe(object) + " accessible", exception);
        }
    }

    static String describe(AccessibleObject object) {
        if (object instanceof Field) {
            Field field = (Field) object;
            return "field '" + field.getDeclaringClass().getName() + "#" + field.getName() + "'";
        } else if (object instanceof Method) {
            Method method = (Method) object;
            return "method '" + method.getDeclaringClass().getName() + "#" + method.getName()
                    + parameters(method) + "'";
        } else if (object instanceof Constructor) {
            Constructor<?> constructor = (Constructor<?>) object;
            return "constructor '" + constructor.getDeclaringClass().getName() + parameters(constructor) + "'";
        }
        return "<unknown AccessibleObject> " + object;
    }

    static String parameters(Executable executable) {
        StringBuilder out = new StringBuilder("(");
        Class<?>[] types = executable.getParameterTypes();
        for (int i = 0; i < types.length; i++) {
            if (i > 0) {
                out.append(", ");
            }
            out.append(types[i].getSimpleName());
        }
        return out.append(") of ").append(executable.getParameterCount()).append(executable.isVarArgs() ? " varargs" : "")
                .toString();
    }

    static String member(Member member) {
        return member.getName() + " " + Modifier.toString(member.getModifiers()) + " synthetic " + member.isSynthetic();
    }

    public static void main(String[] args) throws Exception {
        Class<Point> c = Point.class;
        Field x = c.getField("x");
        Field y = c.getDeclaredField("y");
        Field name = c.getDeclaredField("NAME");
        Method sum = c.getMethod("sum");
        Method describe = c.getDeclaredMethod("describe", String.class, int.class);
        Constructor<Point> plain = Point.class.getConstructor();
        Constructor<Point> both = Point.class.getDeclaredConstructor(int.class, int.class);

        System.out.println(isAccessible(x) + " " + isAccessible(y) + " " + isAccessible(sum) + " " + isAccessible(describe));
        makeAccessible(x);
        makeAccessible(y);
        makeAccessible(name);
        makeAccessible(sum);
        makeAccessible(both);
        forceAccessible(describe);

        AccessibleObject[] all = {x, y, name, sum, describe, plain, both};
        for (AccessibleObject member : all) {
            System.out.println(describe(member) + " can " + member.canAccess(member instanceof Constructor
                    || Modifier.isStatic(((Member) member).getModifiers()) ? null : new Point()));
            System.out.println("  " + member((Member) member) + " " + member.trySetAccessible());
        }
        AccessibleObject.setAccessible(all, true);

        Point p = both.newInstance(3, 4);
        System.out.println(y.get(p) + " " + sum.invoke(p) + " " + describe.invoke(null, "twice", 2));
        Executable[] executables = {sum, describe, plain, both};
        for (Executable e : executables) {
            System.out.println(e.getName() + " " + e.getParameterCount() + " " + e.getExceptionTypes().length);
        }
        try {
            forceAccessible(null);
        } catch (IllegalStateException e) {
            System.out.println(e.getMessage() + ", " + e.getCause().getClass().getName());
        }
    }
}
