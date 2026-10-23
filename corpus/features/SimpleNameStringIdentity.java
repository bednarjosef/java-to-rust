// `==` on simple names: Java keeps a class's simple name once it has made it, so two calls give
// the same object, which is not the literal of the same text.
public class SimpleNameStringIdentity {
    static class Inner {}

    public static void main(String[] args) {
        String first = Inner.class.getSimpleName();
        String second = Inner.class.getSimpleName();
        System.out.println(first == second);
        System.out.println(first == "Inner");
        System.out.println(first.equals("Inner"));
        Object o = new Inner();
        System.out.println(o.getClass().getSimpleName() == first);
        System.out.println(String.class.getSimpleName() == String.class.getSimpleName());
    }
}
