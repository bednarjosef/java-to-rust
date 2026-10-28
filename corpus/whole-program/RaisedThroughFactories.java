import java.io.FileNotFoundException;
import java.io.IOException;
import java.util.ArrayList;
import java.util.List;

/** The runtime's exceptions where the exception classes are built in Java's order. */
public class RaisedThroughFactories {
    /** A constructor that calls a method its subclass overrides: built as a whole object. */
    static class Described extends RuntimeException {
        final List<String> notes = new ArrayList<>();

        Described(String message) {
            super(message);
            describe();
        }

        void describe() {
            notes.add("described");
        }
    }

    static class MoreDescribed extends Described {
        MoreDescribed(String message) {
            super(message);
        }

        @Override
        void describe() {
            System.out.println("more described");
        }
    }

    public static void main(String[] args) throws Exception {
        System.out.println(new Described("plain").notes);
        try {
            throw new MoreDescribed("thrown");
        } catch (Described e) {
            System.out.println("caught " + e.getMessage());
        }
        try {
            if (args.length > 5) {
                throw new FileNotFoundException(args[0]);
            }
            Runtime.getRuntime().exec(new String[] {"no-such-tool-for-the-corpus"});
        } catch (IOException e) {
            System.out.println("caught " + e.getClass().getName() + ": " + e.getMessage());
        }
        try {
            Integer.parseInt(args.length > 7 ? args[0] : "x1");
        } catch (IllegalArgumentException e) {
            System.out.println("caught " + e.getClass().getName() + ": " + e.getMessage());
        }
    }
}
