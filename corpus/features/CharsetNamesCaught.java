// `Charset.forName` and `isSupported` of names that are not a charset's, malformed or null,
// inside `try`s that catch what they throw: `UnsupportedCharsetException`,
// `IllegalCharsetNameException` and `IllegalArgumentException`. As javac's BaseFileManager looks
// up the encoding it is given.
import java.nio.charset.Charset;
import java.nio.charset.IllegalCharsetNameException;
import java.nio.charset.UnsupportedCharsetException;

public class CharsetNamesCaught {
    static String lookUp(String name) {
        try {
            return "found " + Charset.forName(name).name();
        } catch (UnsupportedCharsetException e) {
            return "unsupported " + e.getCharsetName() + " (" + e.getMessage() + ")";
        } catch (IllegalCharsetNameException e) {
            return "illegal " + e.getCharsetName();
        } catch (IllegalArgumentException e) {
            return "bad name: " + e.getMessage();
        }
    }

    static String supported(String name) {
        try {
            return name + " supported: " + Charset.isSupported(name);
        } catch (IllegalArgumentException e) {
            return name + " " + e.getClass().getSimpleName();
        }
    }

    public static void main(String[] args) {
        for (String name : new String[] {"UTF-8", "latin1", "no-such-charset", "bad name!", null}) {
            System.out.println(lookUp(name));
        }
        System.out.println(supported("UTF-16"));
        System.out.println(supported("nope"));
        System.out.println(supported("*"));
    }
}
