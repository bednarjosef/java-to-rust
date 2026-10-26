import java.lang.reflect.Method;
import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.util.Arrays;

// The arrays of members that reflection gives, held as `Object[]`s, as libraries pass them on:
// the reflection table's arrays are then arrays of `Object`s.
public class ReflectionArraysAsObjectArrays {
    static class Point {
        int x;
        int y;

        Point() {}

        Point(int x, int y) {
            this.x = x;
            this.y = y;
        }

        int length() {
            return x * x + y * y;
        }
    }

    static int count(Object[] members) {
        return members.length;
    }

    public static void main(String[] args) throws Exception {
        Object[] constructors = Point.class.getDeclaredConstructors();
        System.out.println(count(constructors));
        int[] parameters = new int[constructors.length];
        for (int i = 0; i < constructors.length; i++) {
            parameters[i] = ((Constructor<?>) constructors[i]).getParameterCount();
        }
        Arrays.sort(parameters);
        System.out.println(Arrays.toString(parameters));
        Object[] fields = Point.class.getDeclaredFields();
        Point p = new Point(3, 4);
        int sum = 0;
        for (Object f : fields) {
            sum += ((Field) f).getInt(p);
        }
        System.out.println(count(fields) + " " + sum);
        Object[] methods = Point.class.getDeclaredMethods();
        System.out.println(count(methods) + " " + ((Method) methods[0]).getName());
    }
}
