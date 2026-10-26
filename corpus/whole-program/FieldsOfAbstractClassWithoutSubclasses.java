import java.lang.reflect.Field;

// The fields of an abstract class that no class of the program extends, so that it has no
// objects: its values are always null, and reading or writing a field of one throws, whether
// the code does it or reflection does (as the reflection table's cases for xalan's
// `AbstractTranslet` do in the Minecraft server's translation).
public class FieldsOfAbstractClassWithoutSubclasses {
    abstract static class Translet {
        int indent;
        String name = "t";
    }

    static int indentOf(Translet t) {
        return t.indent;
    }

    static void rename(Translet t, String name) {
        t.name = name;
    }

    public static void main(String[] args) throws Exception {
        Translet none = args.length > 3 ? null : null;
        try {
            System.out.println(indentOf(none));
        } catch (NullPointerException e) {
            System.out.println("no translet to read");
        }
        try {
            rename(none, "x");
        } catch (NullPointerException e) {
            System.out.println("no translet to write");
        }
        Field indent = Translet.class.getDeclaredField("indent");
        try {
            indent.setInt(new Object(), 3);
        } catch (IllegalArgumentException e) {
            System.out.println("not a translet");
        }
        System.out.println(indent.getName() + " " + indent.getType());
    }
}
