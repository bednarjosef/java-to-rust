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
     * translated program has no call stack to walk (docs/exceptions.md), so this is a native
     * that the translation does not have, rejected where it can run.
     */
    @Replace
    protected Class<?>[] getClassContext() {
        return callStackClasses();
    }

    @Add
    private static native Class<?>[] callStackClasses();
}
