import java.lang.constant.ClassDesc;
import java.lang.invoke.TypeDescriptor;
import java.util.ArrayList;
import java.util.List;

// Classes held as `java.lang.invoke.TypeDescriptor`s, beside a `ClassDesc` and a type of the
// program's own that are ones: `descriptorString()` through the interface, `instanceof Class`,
// casts back, `equals` and `toString`, and an `Object` holding a class tested for one and cast
// to one.
public class ClassesAsTypeDescriptors {
    record Named(String name) implements TypeDescriptor {
        public String descriptorString() {
            return "L" + name + ";";
        }
    }

    static class Inner {}

    static TypeDescriptor pick(int i) {
        return switch (i) {
            case 0 -> String.class;
            case 1 -> int.class;
            case 2 -> long[][].class;
            case 3 -> new Named("x/Y");
            case 4 -> Inner.class;
            case 5 -> ClassDesc.of("java.util", "List");
            case 6 -> null;
            default -> Runnable[].class;
        };
    }

    public static void main(String[] args) {
        for (int i = 0; i < 8; i++) {
            TypeDescriptor type = pick(i);
            if (type == null) {
                System.out.println("null " + (type instanceof Class));
                continue;
            }
            System.out.println(type.descriptorString() + " " + (type instanceof Class) + " " + (type instanceof Named)
                    + " " + type.equals(pick(i)) + " " + type.equals(String.class));
            if (type instanceof Class<?> c) {
                System.out.println("class " + c.getName() + " " + type);
            }
        }
        List<TypeDescriptor> types = new ArrayList<>();
        types.add(boolean.class);
        types.add(Integer.class);
        types.add(new Named("list"));
        for (TypeDescriptor t : types) {
            System.out.println(t.descriptorString());
        }
        System.out.println(types.contains(Integer.class) + " " + types.indexOf(boolean.class) + " " + types.contains(Long.class));
        Object o = args.length == 0 ? Double.class : "text";
        System.out.println((o instanceof TypeDescriptor) + " " + ((TypeDescriptor) o).descriptorString());
        Object s = args.length == 0 ? "text" : Double.class;
        System.out.println(s instanceof TypeDescriptor);
        try {
            Class<?> bad = (Class<?>) pick(3);
            System.out.println(bad);
        } catch (ClassCastException e) {
            System.out.println("cast failed");
        }
    }
}
