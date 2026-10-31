// `Object` has no superclass: `getSuperclass()` of `Object.class`, and of the class of a plain
// `new Object()`, is null, and every chain of superclasses ends there.
public class ObjectHasNoSuperclass {
    static class Animal { }

    static class Dog extends Animal { }

    static int depth(Class<?> c) {
        int n = 0;
        for (Class<?> s = c.getSuperclass(); s != null; s = s.getSuperclass()) {
            n++;
        }
        return n;
    }

    public static void main(String[] args) {
        System.out.println(Object.class.getSuperclass());
        Object plain = new Object();
        System.out.println(plain.getClass().getSuperclass());
        System.out.println(plain.getClass().getSuperclass() == null);
        Object[] values = { plain, new Animal(), new Dog(), "text", 7, new int[1] };
        for (Object v : values) {
            System.out.println(v.getClass().getName() + " " + depth(v.getClass()));
        }
        System.out.println(depth(Object.class) + " " + depth(Dog.class));
    }
}
