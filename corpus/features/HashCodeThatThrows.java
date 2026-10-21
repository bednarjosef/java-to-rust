// A `hashCode` that may throw makes every `hashCode` reached through `Object` one that may: the
// classes that keep `Object`'s, and the strings and boxes held as objects, are hashed as usual.
public class HashCodeThatThrows {
    static class Checked {
        final int divisor;

        Checked(int divisor) {
            this.divisor = divisor;
        }

        @Override
        public int hashCode() {
            if (divisor < 0) {
                throw new IllegalStateException("negative divisor " + divisor);
            }
            return 100 / divisor;
        }
    }

    static class Plain {
        final int value;

        Plain(int value) {
            this.value = value;
        }
    }

    static String hash(Object o) {
        try {
            return "hash " + o.hashCode();
        } catch (IllegalStateException e) {
            return "refused: " + e.getMessage();
        } catch (ArithmeticException e) {
            return "arithmetic: " + e.getMessage();
        }
    }

    public static void main(String[] args) {
        Plain p = new Plain(3);
        Object[] objects = {new Checked(7), new Checked(-1), new Checked(0), "text", 42, p};
        for (Object o : objects) {
            String text = hash(o);
            // A plain object's identity hash differs from run to run.
            System.out.println(o == p ? (text.startsWith("hash ") ? "plain hashed" : text) : text);
        }
    }
}
