import java.util.function.Supplier;

// A `StringBuilder` held as an `Object` value: from a `Supplier<StringBuilder>`, in a bag, and in
// an `Object` variable. It stays the same mutable builder, compared by identity, and prints as its
// text.
public class StringBuildersAsObjects {
    static class Bag<T> {
        Object[] items = new Object[4];
        int size;

        void add(T item) {
            items[size++] = item;
        }

        @SuppressWarnings("unchecked")
        T get(int i) {
            return (T) items[i];
        }
    }

    static String describe(Object o) {
        if (o instanceof StringBuilder sb) {
            return "builder of " + sb.length();
        }
        return "other " + o;
    }

    public static void main(String[] args) {
        Supplier<StringBuilder> builders = StringBuilder::new;
        StringBuilder sb = builders.get();
        sb.append("abc").append(1);
        System.out.println(sb);

        Bag<StringBuilder> list = new Bag<>();
        list.add(sb);
        list.add(new StringBuilder("xy"));
        list.get(0).append('!');
        System.out.println(sb + " " + list.get(1).reverse());

        Object held = sb;
        StringBuilder back = (StringBuilder) held;
        back.append("?");
        System.out.println(held + " " + (held == sb) + " " + held.equals(new StringBuilder("abc1!?")));
        System.out.println(held.hashCode() == sb.hashCode());
        System.out.println(describe(held) + ", " + describe("text") + ", " + describe(7));
    }
}
