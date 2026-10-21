// `Object`'s `toString` shows the object's `hashCode`, here one that may throw and whose
// exception a direct call catches: making the object's text, directly, as an `Object` and in a
// concatenation, calls it too.
public class DefaultTextOfThrowingHashCode {
    static class Key {
        final int[] parts;

        Key(int[] parts) {
            this.parts = parts;
        }

        @Override
        public int hashCode() {
            if (parts.length == 0) {
                throw new IllegalStateException("no parts");
            }
            return parts[0] * 31 + parts.length;
        }
    }

    static class Named {
        @Override
        public String toString() {
            return "named";
        }
    }

    public static void main(String[] args) {
        try {
            System.out.println(new Key(new int[0]).hashCode());
        } catch (IllegalStateException e) {
            System.out.println("caught " + e.getMessage());
        }
        Key key = new Key(new int[] {2, 5});
        System.out.println(key);
        System.out.println("key " + key);
        Object held = args.length > 3 ? new Named() : key;
        System.out.println(held + " " + held.toString().length());
        System.out.println(new Named());
    }
}
