package jdk.internal.misc;

import java.lang.reflect.Modifier;

/**
 * The translator's patch of the JDK's {@code jdk.internal.misc.Unsafe} (see docs/jdk-overlays.md).
 * {@code allocateInstance} makes an object without running a constructor, which a translation
 * cannot do: its objects are made only by their constructors. It throws Java's
 * {@code InstantiationException} for a class that has no objects, and the same exception, saying
 * why, for one that has.
 */
class Unsafe {
    @Replace
    public Object allocateInstance(Class<?> cls) throws InstantiationException {
        if (cls.isInterface() || cls.isArray() || cls.isPrimitive() || Modifier.isAbstract(cls.getModifiers())) {
            throw new InstantiationException(cls.getName());
        }
        throw new InstantiationException(
                cls.getName() + " (a translated program makes no object without running a constructor)");
    }
}
