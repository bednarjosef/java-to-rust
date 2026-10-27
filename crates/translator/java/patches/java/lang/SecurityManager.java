package java.lang;

/**
 * The translator's patch of the JDK's {@code SecurityManager} (see docs/jdk-overlays.md). There
 * is no security manager, as in Java ({@code System.getSecurityManager()} is null), but a class of
 * the program may extend this one, as log4j's and slf4j's do to read the classes on the call
 * stack, and its objects are made as Java makes them.
 */
class SecurityManager {
    /**
     * The classes of the methods on the call stack, which the JDK walks the stack for. A
     * translated program has no call stack to walk (docs/exceptions.md), so this throws, as a
     * JDK without the method would fail: log4j and slf4j call it where they catch the failure
     * and fall back. A census lists it (`program/census.rs`, by this message).
     */
    @Replace
    protected Class<?>[] getClassContext() {
        throw new UnsupportedOperationException(
                "SecurityManager.getClassContext: the classes on the call stack, which a translated program does not have");
    }
}
