// Arrays.equals and Arrays.hashCode of String[] fields that are null, where one of them is also
// held as an Object[] (a covariant array, which the translation copies for the String
// builtins): a null array is equal to a null one and hashes to 0, and a catch reads the message
// of the NullPointerException of the field of a null object.
import java.util.Arrays;

public class ArraysOfNullFields {
    String[] names;
    String[] others;

    ArraysOfNullFields(String[] names, String[] others) {
        this.names = names;
        this.others = others;
    }

    boolean same(ArraysOfNullFields that) {
        return Arrays.equals(names, that.names) && Arrays.equals(others, that.others);
    }

    int hash() {
        return 11 * Arrays.hashCode(names) + Arrays.hashCode(others);
    }

    public static void main(String[] args) {
        ArraysOfNullFields a = new ArraysOfNullFields(null, new String[] {"a"});
        ArraysOfNullFields b = new ArraysOfNullFields(null, new String[] {"a"});
        Object[] covariant = a.others;
        System.out.println(covariant.length);
        try {
            System.out.println(a.same(b) + " " + a.hash());
            System.out.println(a.same(null));
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
    }
}
