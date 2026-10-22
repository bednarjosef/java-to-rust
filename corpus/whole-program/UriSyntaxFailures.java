import java.net.URI;
import java.net.URISyntaxException;

// The JDK's exception constructors test their arguments (`URISyntaxException` throws for a null
// input or an index below -1), which the JDK's code never fails: the URI parser's failures are
// caught here with their messages. The program's own exception constructor that tests its
// argument is translated as any other, where nothing catches what it throws.
public class UriSyntaxFailures {
    static class Rejected extends Exception {
        final int code;

        Rejected(int code) {
            super("rejected " + code);
            if (code < 0) {
                throw new IllegalArgumentException("negative code");
            }
            this.code = code;
        }
    }

    static void check(String text) throws Rejected {
        if (text.contains(" ")) {
            throw new Rejected(text.indexOf(' '));
        }
    }

    public static void main(String[] args) {
        for (String text : new String[] {"jrt:/", "http://example.org/a b", "a%zz", "http://[::1"}) {
            try {
                URI uri = new URI(text);
                System.out.println(uri.getScheme() + " " + uri.getPath());
            } catch (URISyntaxException e) {
                System.out.println(e.getIndex() + " " + e.getReason() + ": " + e.getMessage());
            }
        }
        System.out.println(URI.create("jrt:/java.base/java/lang").getPath());
        try {
            check("no-spaces");
            check("one space");
        } catch (Rejected e) {
            System.out.println(e.getMessage() + " " + e.code);
        }
    }
}
