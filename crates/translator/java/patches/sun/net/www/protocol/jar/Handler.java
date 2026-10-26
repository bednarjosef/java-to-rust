package sun.net.www.protocol.jar;

/**
 * The translator's patch of the JDK's handler of {@code jar:} URLs (see docs/jdk-overlays.md).
 * Reading a jar's entry through its URL needs the JDK's checks of a signed jar, which are
 * security code and not translated, so opening a connection to one throws, saying so. Every URL
 * that a program makes reaches this handler, through the JDK's table of the built-in protocols,
 * and a connection to any URL would otherwise reach the jar files' signature checks.
 */
class Handler {
    @Replace
    protected java.net.URLConnection openConnection(URL u) throws IOException {
        throw new UnsupportedOperationException("the jar " + u + ": code that is not in the translation cannot run");
    }
}
