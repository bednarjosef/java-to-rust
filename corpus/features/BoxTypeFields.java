/** The static field {@code TYPE} of each boxed class is the class of its primitive. */
public class BoxTypeFields {
    static void show(Class<?> c) {
        System.out.println(c + " " + c.isPrimitive() + " " + c.getName());
    }

    public static void main(String[] args) {
        show(Boolean.TYPE);
        show(Byte.TYPE);
        show(Character.TYPE);
        show(Short.TYPE);
        show(Integer.TYPE);
        show(Long.TYPE);
        show(Float.TYPE);
        show(Double.TYPE);
        show(Void.TYPE);

        System.out.println(Boolean.TYPE == boolean.class);
        System.out.println(Integer.TYPE == int.class);
        System.out.println(Integer.TYPE == Integer.class);
        Class<?> c = args.length == 0 ? Character.TYPE : Byte.TYPE;
        System.out.println(c == char.class);
    }
}
