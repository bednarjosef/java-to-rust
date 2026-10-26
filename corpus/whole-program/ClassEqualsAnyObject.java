// `Class.equals` of an argument typed `Object` or `java.lang.reflect.Type`, as log4j's
// `TypeUtil.isAssignable` compares `Object.class.equals(lhs)`: true only for the same class. A
// method returning such a test or another (`isObjectOrString`) ends in a block and an operator.
import java.lang.reflect.Type;

public class ClassEqualsAnyObject {
    static int calls;

    static Type next(Type t) {
        calls++;
        return t;
    }

    static boolean isObject(Type t) {
        return Object.class.equals(t);
    }

    static boolean isObjectOrString(Type t) {
        return Object.class.equals(t) || String.class.equals(t);
    }

    public static void main(String[] args) {
        System.out.println(isObject(Object.class) + " " + isObject(String.class) + " " + isObject(null));
        System.out.println(isObjectOrString(String.class) + " " + isObjectOrString(Integer.class));
        Object text = "text";
        Object klass = Integer.class;
        System.out.println(String.class.equals(text) + " " + Integer.class.equals(klass));
        System.out.println(Integer.class.equals(next(Integer.class)) + " " + calls);
    }
}
