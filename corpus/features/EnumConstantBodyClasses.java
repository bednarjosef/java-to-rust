// An enum constant with a body is of a class of its own, an anonymous subclass of the enum
// (`Op$1`): its `getClass()`, from the enum's type and as an `Object`, its name, simple name,
// superclass and nesting, while `getDeclaringClass()` is the enum for every constant.
public class EnumConstantBodyClasses {
    enum Op {
        ADD { int apply(int a, int b) { return a + b; } },
        SUB { int apply(int a, int b) { return a - b; } },
        NONE;

        int apply(int a, int b) { return 0; }
    }

    enum Plain { A, B }

    static boolean validKey(Object key, Class<?> keyType) {
        Class<?> keyClass = key.getClass();
        return keyClass == keyType || keyClass.getSuperclass() == keyType;
    }

    public static void main(String[] args) {
        for (Op op : Op.values()) {
            Class<?> c = op.getClass();
            System.out.println(op + " " + op.apply(5, 3) + " " + c.getName() + " [" + c.getSimpleName() + "] "
                + c.getSuperclass().getName() + " anonymous " + c.isAnonymousClass() + " enum " + c.isEnum()
                + " declaring " + op.getDeclaringClass().getSimpleName() + " same " + (c == Op.class)
                + " valid " + validKey(op, Op.class));
        }
        Object[] values = { Op.ADD, Op.SUB, Op.NONE, Plain.A, "text" };
        for (Object v : values) {
            System.out.println(v.getClass().getName() + " " + (v.getClass() == Op.class) + " "
                + (v.getClass() == Op.ADD.getClass()) + " " + v.getClass().getSimpleName().isEmpty());
        }
        System.out.println(Op.ADD.getClass() == Op.SUB.getClass());
        System.out.println(Op.ADD.getDeclaringClass() == Op.NONE.getDeclaringClass());
        System.out.println(Plain.A.getClass() == Plain.class);
    }
}
