import java.util.Arrays;

// The range methods of `Arrays` whose range is the wrong way round or outside the array, caught;
// a local assigned in a `try` that may hold null; a `toString` that returns null, shown through
// `Object`; and a method that makes an array of a size that may be negative, whose result is kept
// as an `Object`.
public class CaughtRangesAndNullLocals {
    static class Node {
        final String name;

        Node(String name) {
            this.name = name;
        }

        @Override
        public String toString() {
            return name;
        }
    }

    static Node find(String[] names, String wanted) {
        for (String n : names) {
            if (n.equals(wanted)) {
                return new Node(n);
            }
        }
        if (wanted.isEmpty()) {
            throw new IllegalStateException("nothing wanted");
        }
        return null;
    }

    static int[] squares(int n) {
        int[] out = new int[n];
        for (int i = 0; i < n; i++) {
            out[i] = i * i;
        }
        return out;
    }

    public static void main(String[] args) {
        int[] a = {5, 3, 9, 1, 7};
        try {
            Arrays.fill(a, 3, 1, 0);
        } catch (IllegalArgumentException e) {
            System.out.println("fill: " + e.getMessage());
        }
        try {
            Arrays.sort(a, 0, 10);
        } catch (ArrayIndexOutOfBoundsException e) {
            System.out.println("sort: " + e.getMessage());
        }
        try {
            System.out.println(Arrays.toString(Arrays.copyOfRange(a, 1, 3)));
            System.out.println(Arrays.toString(Arrays.copyOfRange(a, 3, 2)));
        } catch (IllegalArgumentException e) {
            System.out.println("copy: " + e.getMessage());
        }
        Arrays.sort(a, 1, 4);
        System.out.println(Arrays.toString(a));

        String[] names = {"ant", "bee"};
        for (String wanted : new String[] {"bee", "cat", ""}) {
            Node found;
            try {
                found = find(names, wanted);
            } catch (IllegalStateException e) {
                System.out.println("caught " + e.getMessage());
                continue;
            }
            Object shown = found;
            System.out.println(wanted + " -> " + shown + " " + (found == null));
        }
        Object nameless = new Node(null);
        System.out.println(nameless);
        System.out.println("[" + nameless + "]");

        for (int n : new int[] {3, -1}) {
            try {
                Object made = squares(n);
                System.out.println(Arrays.toString((int[]) made));
            } catch (NegativeArraySizeException e) {
                System.out.println("negative " + e.getMessage());
            }
        }
    }
}
