// `==` on class names: HotSpot interns a class's name, so `getName()` is the literal of the same
// text, while `toString()` makes a new string each time.
public class ClassNameStringIdentity {
    static class Inner {}

    public static void main(String[] args) {
        System.out.println(String.class.getName() == "java.lang.String");
        System.out.println(Inner.class.getName() == "ClassNameStringIdentity$Inner");
        Object o = new Inner();
        System.out.println(o.getClass().getName() == Inner.class.getName());
        System.out.println(o.getClass().getName() == "Inner");
        System.out.println(Inner.class.toString() == Inner.class.toString());
        String name = new String("java.lang.String");
        System.out.println(String.class.getName() == name);
        System.out.println(String.class.getName() == name.intern());
    }
}
