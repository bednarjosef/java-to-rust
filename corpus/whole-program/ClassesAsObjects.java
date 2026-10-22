// Classes held as `Object`s: a sentinel compared by identity, two compared with `==`, keys of a map and a set, elements
// of a list and an array, `instanceof Class`, casts back (one that fails), and their hash codes,
// which agree with `System.identityHashCode`, as there is one object for each class.
import java.util.*;
public class ClassesAsObjects {
    static final Object MARK = ClassesAsObjects.class;
    interface Shape {}
    record Circle(int r) implements Shape {}
    static Object pick(int i) {
        return switch (i) { case 0 -> String.class; case 1 -> "text"; case 2 -> 42; case 3 -> Shape.class; default -> new int[0].getClass(); };
    }
    static Object classOf(Object value) {
        return value.getClass();
    }
    public static void main(String[] args) {
        Object first = classOf("a"), second = classOf("b"), third = classOf(3);
        System.out.println((first == second) + " " + (first == third) + " " + (classOf(new Circle(2)) == classOf(new Circle(3))));
        Object o = args.length > 5 ? "x" : MARK;
        System.out.println(o == MARK);
        System.out.println(o);
        Map<Class<?>, String> m = new HashMap<>();
        m.put(String.class, "s");
        m.put(Integer.class, "i");
        System.out.println(m.get(String.class) + m.get(Integer.class) + m.get(ClassesAsObjects.class));
        List<Object> l = new ArrayList<>();
        l.add(ClassesAsObjects.class);
        l.add("str");
        System.out.println(l + " " + l.get(0).equals(ClassesAsObjects.class) + " " + (l.get(0).hashCode() == ClassesAsObjects.class.hashCode()));
        for (int i = 0; i < 5; i++) {
            Object p = pick(i);
            if (p instanceof Class<?> c) {
                System.out.println("class " + c.getSimpleName() + " " + c.isInterface() + " " + p.getClass().getName());
            } else {
                System.out.println("other " + p + " " + p.equals(String.class));
            }
            System.out.println(String.valueOf(p).length() > 0);
        }
        Set<Class<?>> seen = new LinkedHashSet<>(List.of(Circle.class, Shape.class));
        seen.add(Circle.class);
        System.out.println(seen + " " + seen.contains(new Circle(1).getClass()));
        Object[] arr = { Circle.class, 1, "a" };
        Class<?> back = (Class<?>) arr[0];
        System.out.println(back.getName() + " " + Objects.equals(arr[0], Circle.class) + " " + Arrays.asList(arr).indexOf(Circle.class));
        System.out.println(System.identityHashCode(arr[0]) == System.identityHashCode(Circle.class));
        try {
            Class<?> bad = (Class<?>) arr[1];
            System.out.println(bad);
        } catch (ClassCastException e) {
            System.out.println("cast failed");
        }
    }
}
