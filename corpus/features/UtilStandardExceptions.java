// The standard exceptions of `java.util`: `InputMismatchException` (a `NoSuchElementException`),
// `EmptyStackException` and `MissingResourceException`, thrown and caught as themselves and as
// their superclasses, with their messages, causes, `toString` and the fields of the last one.
import java.util.EmptyStackException;
import java.util.InputMismatchException;
import java.util.MissingResourceException;
import java.util.NoSuchElementException;

public class UtilStandardExceptions {
    static int parse(String text) {
        if (text.isEmpty()) {
            throw new InputMismatchException();
        }
        for (int i = 0; i < text.length(); i++) {
            if (text.charAt(i) < '0' || text.charAt(i) > '9') {
                throw new InputMismatchException("not a number: " + text);
            }
        }
        return Integer.parseInt(text);
    }

    static int top(int[] stack, int size) {
        if (size == 0) {
            throw new EmptyStackException();
        }
        return stack[size - 1];
    }

    static String lookup(String bundle, String key) {
        if (!key.startsWith("app.")) {
            throw new MissingResourceException("no value for " + key, bundle, key);
        }
        return "value of " + key;
    }

    static void rethrow(String text) {
        try {
            parse(text);
        } catch (InputMismatchException e) {
            throw new NoSuchElementException("rethrown: " + e.getMessage());
        }
    }

    public static void main(String[] args) {
        String[] inputs = {"42", "", "4x"};
        for (String in : inputs) {
            try {
                System.out.println("read " + parse(in));
            } catch (InputMismatchException e) {
                System.out.println(e.getMessage());
                System.out.println(e.getCause());
                System.out.println(e);
                System.out.println(e instanceof NoSuchElementException);
            }
        }

        try {
            rethrow("x");
        } catch (NoSuchElementException e) {
            System.out.println(e.getClass().getName() + ": " + e.getMessage());
        }

        NoSuchElementException superclass = new InputMismatchException("as its superclass");
        System.out.println(superclass.getMessage() + " / " + superclass);

        try {
            System.out.println(top(new int[] {1, 2, 3}, 3));
            System.out.println(top(new int[] {1, 2, 3}, 0));
        } catch (EmptyStackException e) {
            System.out.println("empty: " + e.getMessage());
            System.out.println("cause: " + e.getCause());
            System.out.println("string: " + e);
        } finally {
            System.out.println("stack checked");
        }

        try {
            throw new EmptyStackException();
        } catch (RuntimeException e) {
            System.out.println("as RuntimeException: " + e.getClass().getSimpleName());
        }

        try {
            System.out.println(lookup("messages", "app.title"));
            System.out.println(lookup("messages", "help.text"));
        } catch (MissingResourceException e) {
            System.out.println(e.getMessage());
            System.out.println(e.getClassName() + " / " + e.getKey());
            System.out.println(e);
        }

        MissingResourceException direct = new MissingResourceException("direct", "bundle", "key");
        System.out.println(direct.getCause() == null);
        System.out.println(direct.getClassName().length() + direct.getKey().length());
        RuntimeException plain = direct;
        System.out.println(plain.getMessage());
    }
}
