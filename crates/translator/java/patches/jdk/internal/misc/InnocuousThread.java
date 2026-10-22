package jdk.internal.misc;

/**
 * Patched because the JDK's static initializer reads the parent of each thread group through an
 * {@code Unsafe} offset held in a local, which a translation does not support. {@code
 * ThreadGroup.getParent} reads the same field. The offsets of {@code Thread} stay: the translation
 * reads and writes those fields directly, so the constructor and {@code eraseThreadLocals} are
 * the JDK's own code.
 */
@Edit(find = "Class<?> gk = ThreadGroup.class;", replace = "")
@Edit(find = "long gp = UNSAFE.objectFieldOffset(gk, \"parent\");", replace = "")
@Edit(find = "(ThreadGroup)UNSAFE.getReference(group, gp);", replace = "group.getParent();")
class InnocuousThread {
}
