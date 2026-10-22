import java.util.function.BiPredicate;

// A lambda of an interface that a class implements too, in a field of an anonymous class, as
// javac's Resolve has: the lambda's class is named after the anonymous class's place.
public class LambdaInAnonymousField {
    static abstract class Template {
        abstract boolean matches(Object o);
    }

    static class Positive implements BiPredicate<Object, Integer> {
        public boolean test(Object o, Integer n) {
            return n > 0;
        }
    }

    static final Template longWords = new Template() {
        final BiPredicate<Object, Integer> longer = (o, n) -> o.toString().length() > n;

        boolean matches(Object o) {
            return longer.test(o, 4);
        }
    };

    public static void main(String[] args) {
        BiPredicate<Object, Integer> positive = new Positive();
        System.out.println(longWords.matches("short") + " " + longWords.matches("tiny") + " " + positive.test("", 1));
    }
}
