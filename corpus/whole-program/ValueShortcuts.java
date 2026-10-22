// An identity test of the JDK on objects that may be strings or boxes where comparing values
// gives Java's answer: `Arrays.deepEquals`, whose `e1 == e2` only skips the `equals` of the two,
// and `Objects.equals`, with the boxes of floating values, which `equals` compares by their bits.
import java.util.Arrays;
import java.util.Objects;

public class ValueShortcuts {
    public static void main(String[] args) {
        String built = new StringBuilder("a").append("b").toString();
        Object[] first = {"ab", 1, 2.5, new Object[] {"x", 'c'}, null};
        Object[] second = {built, 1, 2.5, new Object[] {"x", 'c'}, null};
        System.out.println(Arrays.deepEquals(first, second));
        System.out.println(Arrays.deepEquals(new Object[] {0.0}, new Object[] {-0.0}));
        System.out.println(Arrays.deepEquals(new Object[] {Double.NaN}, new Object[] {Double.NaN}));
        System.out.println(Arrays.deepEquals(new Object[] {1}, new Object[] {1L}));
        System.out.println(Arrays.deepEquals(new Object[] {"ab"}, new Object[] {"ba"}));
        Object zero = 0.0f;
        Object negative = -0.0f;
        Object nan = Double.NaN;
        System.out.println(Objects.equals(zero, negative) + " " + Objects.equals(nan, Double.NaN) + " " + Objects.equals(zero, 0.0f));
    }
}
