package java.lang;

import java.security.AccessControlException;
import java.security.Permission;
import java.security.PrivilegedAction;
import java.security.PrivilegedActionException;
import java.security.PrivilegedExceptionAction;

/**
 * The translator's {@code AccessController.doPrivileged(action)},
 * {@code doPrivilegedWithCombiner(action)} and {@code checkPermission}: a translated program has no
 * security manager, so an action that would run with its caller's privileges just runs, and every
 * permission check fails, as in a JDK without one. An action that may throw a checked exception
 * has it wrapped in a {@code PrivilegedActionException}, while an unchecked one passes through, as
 * the JDK's does. The JDK's {@code AccessController} is not translated.
 */
final class TranslatedAccessController {
    private TranslatedAccessController() {}

    static <T> T doPrivileged(PrivilegedAction<T> action) {
        return action.run();
    }

    static <T> T doPrivilegedWithCombiner(PrivilegedAction<T> action) {
        return action.run();
    }

    static <T> T doPrivileged(PrivilegedExceptionAction<T> action) throws PrivilegedActionException {
        try {
            return action.run();
        } catch (RuntimeException e) {
            throw e;
        } catch (Exception e) {
            throw new PrivilegedActionException(e);
        }
    }

    static <T> T doPrivilegedWithCombiner(PrivilegedExceptionAction<T> action)
            throws PrivilegedActionException {
        return doPrivileged(action);
    }

    @SuppressWarnings("removal")
    static void checkPermission(Permission perm) {
        throw new AccessControlException("checking permissions is not supported");
    }
}
