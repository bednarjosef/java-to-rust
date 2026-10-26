import static java.lang.invoke.MethodType.methodType;

import java.lang.invoke.MethodHandle;
import java.lang.invoke.MethodHandles;

// netty's way with JDK features it may lack: a method handle looked up and tried in a `try`, null
// when that fails, and a fallback for null. The translation, which follows no handle of a JDK
// method that it does not have, takes the fallbacks, and so gives what the JVM gives.
public class UnfollowedHandleLookups {
    private static final MethodHandle IS_VIRTUAL = isVirtualHandle();

    private static MethodHandle isVirtualHandle() {
        try {
            MethodHandle handle =
                    MethodHandles.publicLookup().findVirtual(Thread.class, "isVirtual", methodType(boolean.class));
            boolean isVirtual = (boolean) handle.invokeExact(Thread.currentThread());
            return handle;
        } catch (Throwable e) {
            return null;
        }
    }

    static boolean isVirtual(Thread thread) {
        if (IS_VIRTUAL == null) {
            return false;
        }
        try {
            return (boolean) IS_VIRTUAL.invokeExact(thread);
        } catch (Throwable t) {
            throw new Error(t);
        }
    }

    // The result of a call that throws, used before anything could check it.
    static int nameLength() {
        try {
            MethodHandle current =
                    MethodHandles.publicLookup().findStatic(Thread.class, "currentThread", methodType(Thread.class));
            Thread thread = (Thread) current.invokeExact();
            return thread.getName().length();
        } catch (Throwable t) {
            return Thread.currentThread().getName().length();
        }
    }

    public static void main(String[] args) {
        System.out.println(isVirtual(Thread.currentThread()));
        System.out.println(nameLength());
    }
}
