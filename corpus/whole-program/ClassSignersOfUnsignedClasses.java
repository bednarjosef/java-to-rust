// `Class.getSigners()`, as the Minecraft server's `ModCheck` asks of its own class when it loads
// a world: `null` for a class without signers, which every class of a translation is, as are
// those that `javac` compiles to a directory.
public class ClassSignersOfUnsignedClasses {
    static class Inner {}

    static String check(Class<?> canary) {
        return canary.getSigners() == null ? "jar signature invalidated" : "signature untouched";
    }

    public static void main(String[] args) {
        System.out.println(check(ClassSignersOfUnsignedClasses.class));
        System.out.println(check(new Inner().getClass()));
        Inner.class.getSigners();
        Object[] signers = String.class.getSigners();
        System.out.println(signers == null);
    }
}
