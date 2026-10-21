// An object without its own `toString` whose `hashCode` throws something that a caller catches:
// the text that `Object.toString` makes holds the hash code, and an exception there ends the
// program, as one of a `toString` that an object's text runs does.
public class DefaultTextOfThrowingHash {
    static final class Key {
        final int value;

        Key(int value) {
            this.value = value;
        }

        @Override
        public int hashCode() {
            if (value < 0) {
                throw new IllegalStateException("negative key " + value);
            }
            return value * 31;
        }
    }

    public static void main(String[] args) {
        try {
            System.out.println(new Key(2).hashCode());
            System.out.println(new Key(-1).hashCode());
        } catch (IllegalStateException e) {
            System.out.println("caught " + e.getMessage());
        }
        Object shown = new Key(255);
        System.out.println(new Key(16) + " " + shown);
    }
}
