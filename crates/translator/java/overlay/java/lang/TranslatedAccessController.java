package java.lang;

import java.security.PrivilegedAction;

/**
 * The translator's {@code AccessController.doPrivileged(action)}: a translated program has no
 * security manager, so an action that would run with its caller's privileges just runs, as it
 * does in a JDK without one. The JDK's {@code AccessController} is not translated.
 */
final class TranslatedAccessController {
    private TranslatedAccessController() {}

    static <T> T doPrivileged(PrivilegedAction<T> action) {
        return action.run();
    }
}
