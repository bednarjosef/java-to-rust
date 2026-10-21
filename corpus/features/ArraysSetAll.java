import java.util.Arrays;
import java.util.function.IntFunction;

// `Arrays.setAll` fills an array from a generator of its indexes: objects from a lambda (as Are
// We Fast Yet's DeltaBlue builds its variables), `int`s, `long`s and `double`s from the
// primitive generators, a method reference, a generator held in a variable, and an empty array.
public class ArraysSetAll {
    static final class Variable {
        final int id;

        Variable(int id) {
            this.id = id;
        }

        @Override
        public String toString() {
            return "v" + id;
        }
    }

    static int calls;

    static String label(int i) {
        calls++;
        return "#" + i;
    }

    public static void main(String[] args) {
        Variable[] vars = new Variable[4];
        Arrays.setAll(vars, i -> new Variable(i * 10));
        StringBuilder shown = new StringBuilder();
        for (Variable v : vars) {
            shown.append(v).append(' ');
        }
        System.out.println(shown);

        int[] squares = new int[5];
        Arrays.setAll(squares, i -> i * i);
        System.out.println(Arrays.toString(squares));

        long[] powers = new long[4];
        Arrays.setAll(powers, i -> 1L << (i * 20));
        System.out.println(Arrays.toString(powers));

        double[] halves = new double[3];
        Arrays.setAll(halves, i -> i / 2.0);
        System.out.println(Arrays.toString(halves));

        String[] labels = new String[3];
        Arrays.setAll(labels, ArraysSetAll::label);
        System.out.println(String.join(",", labels) + " " + calls);

        IntFunction<Variable> make = Variable::new;
        Variable[] more = new Variable[2];
        Arrays.setAll(more, make);
        Arrays.setAll(new Variable[0], make);
        System.out.println(more[1] + " " + more.length);
    }
}
