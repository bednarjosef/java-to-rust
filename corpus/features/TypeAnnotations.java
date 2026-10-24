import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;

// Type annotations, in the style of JSpecify's `@Nullable` and Minecraft's `@Block.UpdateFlags`,
// on fields, parameters, results, locals, arrays, type arguments and qualified names.
public class TypeAnnotations {
    @Target(ElementType.TYPE_USE)
    @Retention(RetentionPolicy.RUNTIME)
    @interface Nullable {}

    @Target(ElementType.TYPE_USE)
    @interface Flags {}

    static class Box<T> {
        @Nullable T value;

        Box(@Nullable T value) { this.value = value; }

        @Nullable T get() { return value; }
    }

    static class Node {
        java.lang.@Nullable String name;
        TypeAnnotations.@Nullable Node next;

        Node(java.lang.@Nullable String name, TypeAnnotations.@Nullable Node next) {
            this.name = name;
            this.next = next;
        }
    }

    static final @Flags int UPDATE = 3;

    static int combine(@Flags int a, @Flags int b) { return a | b; }

    static java.lang.@Nullable Object first(@Nullable Object @Nullable [] values) {
        return values == null || values.length == 0 ? null : values[0];
    }

    static int length(Node node) {
        int n = 0;
        for (@Nullable Node at = node; at != null; at = at.next) {
            n++;
        }
        return n;
    }

    public static void main(String[] args) {
        Box<@Nullable String> box = new Box<>("boxed");
        Box<java.lang.@Nullable Integer> empty = new Box<>(null);
        System.out.println(box.get() + " " + empty.get());
        Node list = new Node("a", new Node(null, new Node("c", null)));
        System.out.println(length(list) + " " + list.next.name + " " + list.next.next.name);
        @Flags int flags = combine(UPDATE, 8);
        System.out.println(flags);
        System.out.println(first(new Object[] {"x", 1}) + " " + first(null) + " " + first(new Object[0]));
        @Nullable String @Nullable [] names = {"p", null};
        System.out.println(names.length + " " + names[1]);
    }
}
