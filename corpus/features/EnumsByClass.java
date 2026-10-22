// `Enum.valueOf(Class, String)` of a class known only as a value, as javac's AnnotationProxyMaker
// reads an annotation's enum constant: the constant of the enum the class is, and the JDK's
// failures for a missing constant and for a class that is no enum.
public class EnumsByClass {
    enum Color { RED, GREEN }

    enum Size {
        SMALL, LARGE;

        @Override
        public String toString() {
            return name().toLowerCase();
        }
    }

    @SuppressWarnings({"unchecked", "rawtypes"})
    static Object constant(Class<?> type, String name) {
        try {
            return Enum.valueOf((Class) type, name);
        } catch (IllegalArgumentException e) {
            return "failed: " + e.getMessage();
        }
    }

    public static void main(String[] args) {
        System.out.println(constant(Color.class, "GREEN"));
        System.out.println(constant(Size.class, "LARGE"));
        System.out.println(constant(Color.class, "BLUE"));
        System.out.println(constant(String.class, "RED"));
        Color c = Enum.valueOf(Color.class, "RED");
        System.out.println(c.ordinal());
    }
}
