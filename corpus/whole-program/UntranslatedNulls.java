// Variables of a class that the translation does not have, as `TreeMap.buildFromSorted` takes
// the `ObjectInputStream` that deserialization reads from and every copy of a sorted map passes
// null. No translated code can create such an object, so the variable is only ever null: a
// method called on it throws `NullPointerException`, and `instanceof` is false.
import java.io.IOException;
import java.io.ObjectInputStream;

public class UntranslatedNulls {
    static String describe(ObjectInputStream in, String fallback) throws IOException {
        if (in == null) {
            return fallback;
        }
        return "read " + in.readInt();
    }

    static String passOn(int depth, ObjectInputStream in) throws IOException {
        return depth == 0 ? describe(in, "none at the bottom") : passOn(depth - 1, in);
    }

    static String safely(ObjectInputStream in) {
        try {
            return "read " + in.readInt();
        } catch (NullPointerException e) {
            return "no stream to read";
        } catch (IOException e) {
            return "failed to read";
        }
    }

    public static void main(String[] args) throws IOException {
        System.out.println(describe(null, "none"));
        System.out.println(passOn(3, null));
        System.out.println(safely(null));
        ObjectInputStream stream = null;
        Object value = stream;
        System.out.println(value instanceof ObjectInputStream);
        System.out.println(stream == null);
    }
}
