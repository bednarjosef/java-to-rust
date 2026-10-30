import java.net.MalformedURLException;
import java.net.URI;
import java.net.URL;

// The handler of a URL's protocol, which `URL` finds by a class name it makes of the protocol,
// `"sun.net.www.protocol." + protocol + ".Handler"`: each protocol's default port comes from it,
// and a protocol with no handler is a MalformedURLException.
public class UrlProtocolHandlers {
    public static void main(String[] args) throws Exception {
        String[] urls = {
            "https://example.com:8443/a/b?q=1", "https://example.com/", "http://example.com/x",
            "ftp://example.com/pub", "file:/tmp/x", "jar:file:/tmp/a.jar!/b.txt",
        };
        for (String text : urls) {
            URL url = URI.create(text).toURL();
            System.out.println(url.getProtocol() + " " + url.getHost() + " " + url.getPort() + " "
                + url.getDefaultPort() + " " + url.getPath() + " " + url.getQuery());
        }
        try {
            System.out.println(URI.create("nosuch://example.com/").toURL());
        } catch (MalformedURLException e) {
            System.out.println("MalformedURLException: " + e.getMessage());
        }
    }
}
