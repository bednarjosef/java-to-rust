// `instanceof` of a box or a string on a value of an interface that they implement
// (`ConstantDesc`, `Constable`, `Serializable`), as the JDK's `CodeBuilder.loadConstant` tests a
// `ConstantDesc`; and `getClass()` of a `Class`.
import java.io.Serializable;
import java.lang.constant.Constable;
import java.lang.constant.ConstantDesc;

public class ConstantDescTests {
    static String kind(ConstantDesc value) {
        if (value instanceof Integer i) return "int " + i;
        if (value instanceof Long l) return "long " + l;
        if (value instanceof Float f) return "float " + f;
        if (value instanceof Double d) return "double " + d;
        if (value instanceof String s) return "string " + s;
        return "other";
    }

    static boolean serializableString(Serializable s) {
        return s instanceof String;
    }

    static String constable(Constable c) {
        if (c instanceof Boolean b) return "boolean " + b;
        if (c instanceof Character ch) return "char " + ch;
        if (c instanceof String s) return "string " + s;
        return "other";
    }

    public static void main(String[] args) {
        ConstantDesc[] values = {1, 2L, 3.5f, 4.25, "five"};
        for (ConstantDesc v : values) {
            System.out.println(kind(v));
        }
        System.out.println(serializableString("s") + " " + serializableString(7));
        System.out.println(constable(true) + ", " + constable('c') + ", " + constable("t") + ", " + constable(8));
        Class<?> c = String.class;
        System.out.println(c.getClass() + " " + c.getClass().getName());
    }
}
