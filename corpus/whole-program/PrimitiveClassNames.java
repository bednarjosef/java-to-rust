// `Class.forPrimitiveName`: the class of each primitive type and of `void`, null for any other
// name (a class's name, a wrong case, an array), and the exception for a null name.
public class PrimitiveClassNames {
    public static void main(String[] args) {
        String[] names = { "boolean", "byte", "char", "short", "int", "long", "float", "double", "void",
            "Int", "java.lang.Integer", "int[]", "", "I" };
        for (String name : names) {
            Class<?> c = Class.forPrimitiveName(name);
            System.out.println(name + " -> " + c + (c == null ? "" : " " + c.isPrimitive()));
        }
        System.out.println(Class.forPrimitiveName("int") == int.class);
        System.out.println(Class.forPrimitiveName("void") == void.class);
        String missing = args.length > 5 ? "x" : null;
        try {
            Class.forPrimitiveName(missing);
        } catch (NullPointerException e) {
            System.out.println("NullPointerException");
        }
    }
}
