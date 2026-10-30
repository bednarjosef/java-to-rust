// Array type patterns in a pattern switch, as the JDK's `KeyUtil.clear` zeroes each key part it
// is given: `case byte[] b` and `case char[] c` test the class the array carries.
public class ArrayPatternsInSwitch {
    static class Secret {
        boolean cleared;
        void clear() { cleared = true; }
    }

    static String clear(Object... parts) {
        StringBuilder out = new StringBuilder();
        for (Object o : parts) {
            switch (o) {
                case byte[] b -> { java.util.Arrays.fill(b, (byte) 0); out.append("bytes ").append(b.length); }
                case char[] c -> { java.util.Arrays.fill(c, '\0'); out.append("chars ").append(c.length); }
                case int[][] grid -> out.append("grid ").append(grid.length);
                case Secret s -> { s.clear(); out.append("secret ").append(s.cleared); }
                case null -> out.append("null");
                default -> out.append("other ").append(o);
            }
            out.append("; ");
        }
        return out.toString();
    }

    public static void main(String[] args) {
        byte[] key = { 1, 2, 3 };
        char[] password = { 'p', 'w' };
        System.out.println(clear(key, password, new int[2][3], new Secret(), null, "text", 7));
        System.out.println(key[0] + key[1] + key[2] + " " + (int) password[0]);
        Object o = new String[] { "a" };
        String kind = switch (o) {
            case byte[] b -> "bytes";
            case Object[] objects -> "objects " + objects.length;
            default -> "other";
        };
        System.out.println(kind);
    }
}
