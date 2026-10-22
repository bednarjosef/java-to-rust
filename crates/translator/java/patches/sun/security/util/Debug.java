package sun.security.util;

/**
 * The translator's patch of the JDK's {@code Debug} (see docs/jdk-overlays.md), the debugging
 * output of the security code that the {@code java.security.debug} property turns on, which is
 * translated so that {@code Debug.getInstance} reads the property as the JDK does. A message
 * names the caller's frame ({@code StackWalker}), which the translation does not have, so writing
 * one fails loudly, saying so.
 */
class Debug {
    @Replace
    private String extraInfo() {
        throw new UnsupportedOperationException(
                "the output of java.security.debug, which names the caller's stack frame, is not supported by the translation");
    }
}
