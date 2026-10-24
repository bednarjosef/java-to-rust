// `java.lang.reflect.Method`, `Field` and `Constructor` as the translator provides them, over the
// translation's reflection table: the members of the program's classes that reflection can reach,
// which the translator chose when translating (see `lower/reflection.rs`). `build.rs` prints the
// syntax tree of this file with `JavaTree`, and lowering gives its classes their real package and
// adds them, with the members a program uses, to the programs that use them, as it adds the
// standard exception classes (see `lower/exceptions.rs`). Each member has a slot in the table;
// the natives `table0`, `invoke0`, `get0`, `set0` and `newInstance0` are written by lowering for
// each program: the table's members, and a `switch` over the slots that calls the member of each.
// A lookup by a name that is not in the table throws Java's exception, with a message that says
// so. An exception that the member throws is wrapped in an `InvocationTargetException`, as in
// Java, except by `Class.newInstance`, which throws it as it is. One deviation: a final field is
// never set, even after `setAccessible(true)`: `set` throws `IllegalAccessException`.
package reflection;

import java.lang.reflect.InvocationTargetException;

final class Method {
    // The table, made once.
    private static Method[] table;

    // The class that `getMethod` and `getDeclaredMethod` find the method on: the declaring class,
    // or for a public method a class that inherits it.
    private final Class<?> owner;
    private final Class<?> clazz;
    private final String name;
    private final Class<?>[] parameterTypes;
    private final Class<?> returnType;
    private final int modifiers;
    private final int slot;

    Method(Class<?> owner, Class<?> clazz, String name, Class<?>[] parameterTypes, Class<?> returnType,
            int modifiers, int slot) {
        this.owner = owner;
        this.clazz = clazz;
        this.name = name;
        this.parameterTypes = parameterTypes;
        this.returnType = returnType;
        this.modifiers = modifiers;
        this.slot = slot;
    }

    public Class<?> getDeclaringClass() {
        return clazz;
    }

    public String getName() {
        return name;
    }

    public int getModifiers() {
        return modifiers;
    }

    public Class<?> getReturnType() {
        return returnType;
    }

    public Class<?>[] getParameterTypes() {
        return Constructor.copy(parameterTypes);
    }

    public int getParameterCount() {
        return parameterTypes.length;
    }

    public boolean isVarArgs() {
        return (modifiers & 0x80) != 0;
    }

    // The table holds the methods that the sources declare, none of which is synthetic.
    public boolean isSynthetic() {
        return (modifiers & 0x1000) != 0;
    }

    public boolean isDefault() {
        return (modifiers & (0x400 | 1 | 8)) == 1 && clazz.isInterface();
    }

    public void setAccessible(boolean flag) {
    }

    public boolean trySetAccessible() {
        return true;
    }

    public boolean canAccess(Object obj) {
        return true;
    }

    public Object invoke(Object obj, Object... args) throws IllegalAccessException, InvocationTargetException {
        Object[] arguments = Constructor.arguments(parameterTypes, args);
        if ((modifiers & 8) == 0) {
            if (obj == null) {
                throw new NullPointerException("Cannot invoke \"Object.getClass()\" because \"obj\" is null");
            }
            if (!clazz.isInstance(obj)) {
                throw new IllegalArgumentException("object is not an instance of declaring class");
            }
        }
        Object result;
        try {
            result = invoke0(slot, obj, arguments);
        } catch (Throwable t) {
            throw new InvocationTargetException(t);
        }
        return result;
    }

    public boolean equals(Object o) {
        if (!(o instanceof Method)) {
            return false;
        }
        Method other = (Method) o;
        return clazz == other.clazz && name.equals(other.name) && returnType == other.returnType
                && Constructor.same(parameterTypes, other.parameterTypes);
    }

    public int hashCode() {
        return clazz.getName().hashCode() ^ name.hashCode();
    }

    public String toString() {
        return Constructor.modifierText(modifiers & ~0x80) + Constructor.typeName(returnType) + " "
                + Constructor.typeName(clazz) + "." + name + "(" + Constructor.typeNames(parameterTypes) + ")";
    }

    // `c.getMethod(name, types)`, or with `declared` `c.getDeclaredMethod(name, types)`.
    static Method find(Class<?> c, String name, Class<?>[] types, boolean declared) throws NoSuchMethodException {
        annotationMembers(c);
        Method[] all = table();
        for (int i = 0; i < all.length; i++) {
            Method m = all[i];
            if (m.owner == c && m.name.equals(name) && m.found(c, declared)
                    && Constructor.same(m.parameterTypes, types)) {
                return m;
            }
        }
        throw new NoSuchMethodException(c.getName() + "." + name + "(" + Constructor.names(types) + ")"
                + Constructor.MISSING);
    }

    // `c.getMethods()`, or with `declared` `c.getDeclaredMethods()`: those of the table.
    static Method[] list(Class<?> c, boolean declared) {
        annotationMembers(c);
        Method[] all = table();
        int n = 0;
        for (int i = 0; i < all.length; i++) {
            if (all[i].owner == c && all[i].found(c, declared)) {
                n++;
            }
        }
        Method[] out = new Method[n];
        n = 0;
        for (int i = 0; i < all.length; i++) {
            if (all[i].owner == c && all[i].found(c, declared)) {
                out[n] = all[i];
                n++;
            }
        }
        return out;
    }

    // An annotation type is not translated, so its members, which an annotation object's values
    // are read by (javac's `AnnotationProxyMaker`), are not in the table: looking them up fails
    // loudly rather than as a member that the type does not have.
    private static void annotationMembers(Class<?> c) {
        if (c != null && c.isAnnotation()) {
            throw new UnsupportedOperationException("the methods of the annotation type " + c.getName()
                    + ", which is not in the translation");
        }
    }

    private boolean found(Class<?> c, boolean declared) {
        return declared ? clazz == c : (modifiers & 1) != 0;
    }

    private static Method[] table() {
        if (table == null) {
            table = table0();
        }
        return table;
    }

    private static native Method[] table0();

    private static native Object invoke0(int slot, Object obj, Object[] args);
}

final class Field {
    private static Field[] table;

    private final Class<?> owner;
    private final Class<?> clazz;
    private final String name;
    private final Class<?> type;
    private final int modifiers;
    private final int slot;

    Field(Class<?> owner, Class<?> clazz, String name, Class<?> type, int modifiers, int slot) {
        this.owner = owner;
        this.clazz = clazz;
        this.name = name;
        this.type = type;
        this.modifiers = modifiers;
        this.slot = slot;
    }

    public Class<?> getDeclaringClass() {
        return clazz;
    }

    public String getName() {
        return name;
    }

    public int getModifiers() {
        return modifiers;
    }

    public Class<?> getType() {
        return type;
    }

    public void setAccessible(boolean flag) {
    }

    public boolean trySetAccessible() {
        return true;
    }

    public boolean canAccess(Object obj) {
        return true;
    }

    public Object get(Object obj) throws IllegalAccessException {
        check(obj);
        return get0(slot, obj);
    }

    public boolean getBoolean(Object obj) throws IllegalAccessException {
        return (Boolean) Constructor.argument(boolean.class, get(obj));
    }

    public byte getByte(Object obj) throws IllegalAccessException {
        return (Byte) Constructor.argument(byte.class, get(obj));
    }

    public char getChar(Object obj) throws IllegalAccessException {
        return (Character) Constructor.argument(char.class, get(obj));
    }

    public short getShort(Object obj) throws IllegalAccessException {
        return (Short) Constructor.argument(short.class, get(obj));
    }

    public int getInt(Object obj) throws IllegalAccessException {
        return (Integer) Constructor.argument(int.class, get(obj));
    }

    public long getLong(Object obj) throws IllegalAccessException {
        return (Long) Constructor.argument(long.class, get(obj));
    }

    public float getFloat(Object obj) throws IllegalAccessException {
        return (Float) Constructor.argument(float.class, get(obj));
    }

    public double getDouble(Object obj) throws IllegalAccessException {
        return (Double) Constructor.argument(double.class, get(obj));
    }

    // A final field keeps its value: the translation relies on it (a final field of a class
    // that threads share is read without a lock, for one).
    public void set(Object obj, Object value) throws IllegalAccessException {
        check(obj);
        if ((modifiers & 0x10) != 0) {
            throw new IllegalAccessException("Can not set final " + Constructor.typeName(type) + " field "
                    + clazz.getName() + "." + name + ": the translation does not change final fields");
        }
        Object written = set0(slot, obj, Constructor.argument(type, value));
    }

    public void setBoolean(Object obj, boolean z) throws IllegalAccessException {
        set(obj, z);
    }

    public void setByte(Object obj, byte b) throws IllegalAccessException {
        set(obj, b);
    }

    public void setChar(Object obj, char c) throws IllegalAccessException {
        set(obj, c);
    }

    public void setShort(Object obj, short s) throws IllegalAccessException {
        set(obj, s);
    }

    public void setInt(Object obj, int i) throws IllegalAccessException {
        set(obj, i);
    }

    public void setLong(Object obj, long l) throws IllegalAccessException {
        set(obj, l);
    }

    public void setFloat(Object obj, float f) throws IllegalAccessException {
        set(obj, f);
    }

    public void setDouble(Object obj, double d) throws IllegalAccessException {
        set(obj, d);
    }

    private void check(Object obj) {
        if ((modifiers & 8) == 0) {
            if (obj == null) {
                throw new NullPointerException("Cannot invoke \"Object.getClass()\" because \"obj\" is null");
            }
            if (!clazz.isInstance(obj)) {
                throw new IllegalArgumentException("Can not get " + Constructor.typeName(type) + " field "
                        + clazz.getName() + "." + name + " on " + obj.getClass().getName());
            }
        }
    }

    public boolean equals(Object o) {
        if (!(o instanceof Field)) {
            return false;
        }
        Field other = (Field) o;
        return clazz == other.clazz && name.equals(other.name) && type == other.type;
    }

    public int hashCode() {
        return clazz.getName().hashCode() ^ name.hashCode();
    }

    public String toString() {
        return Constructor.modifierText(modifiers) + Constructor.typeName(type) + " " + Constructor.typeName(clazz)
                + "." + name;
    }

    // `c.getField(name)`, or with `declared` `c.getDeclaredField(name)`.
    static Field find(Class<?> c, String name, boolean declared) throws NoSuchFieldException {
        Field[] all = table();
        for (int i = 0; i < all.length; i++) {
            Field f = all[i];
            if (f.owner == c && f.name.equals(name) && f.found(c, declared)) {
                return f;
            }
        }
        throw new NoSuchFieldException(name + Constructor.MISSING);
    }

    static Field[] list(Class<?> c, boolean declared) {
        Field[] all = table();
        int n = 0;
        for (int i = 0; i < all.length; i++) {
            if (all[i].owner == c && all[i].found(c, declared)) {
                n++;
            }
        }
        Field[] out = new Field[n];
        n = 0;
        for (int i = 0; i < all.length; i++) {
            if (all[i].owner == c && all[i].found(c, declared)) {
                out[n] = all[i];
                n++;
            }
        }
        return out;
    }

    private boolean found(Class<?> c, boolean declared) {
        return declared ? clazz == c : (modifiers & 1) != 0;
    }

    private static Field[] table() {
        if (table == null) {
            table = table0();
        }
        return table;
    }

    private static native Field[] table0();

    private static native Object get0(int slot, Object obj);

    // Gives the value written: the lowering writes it as an expression, whose value has a type.
    private static native Object set0(int slot, Object obj, Object value);
}

final class Constructor<T> {
    // The text that the exceptions of a lookup by a name not in the table end with.
    static final String MISSING = ", which is not in the translation";

    private static Constructor<?>[] table;

    private final Class<T> clazz;
    private final Class<?>[] parameterTypes;
    private final int modifiers;
    private final int slot;

    Constructor(Class<T> clazz, Class<?>[] parameterTypes, int modifiers, int slot) {
        this.clazz = clazz;
        this.parameterTypes = parameterTypes;
        this.modifiers = modifiers;
        this.slot = slot;
    }

    public Class<T> getDeclaringClass() {
        return clazz;
    }

    public String getName() {
        return clazz.getName();
    }

    public int getModifiers() {
        return modifiers;
    }

    public Class<?>[] getParameterTypes() {
        return copy(parameterTypes);
    }

    public int getParameterCount() {
        return parameterTypes.length;
    }

    public boolean isVarArgs() {
        return (modifiers & 0x80) != 0;
    }

    public void setAccessible(boolean flag) {
    }

    public boolean trySetAccessible() {
        return true;
    }

    public boolean canAccess(Object obj) {
        return true;
    }

    // The slot of a constructor of an abstract class is negative: there is nothing to create.
    @SuppressWarnings("unchecked")
    public T newInstance(Object... initargs)
            throws InstantiationException, IllegalAccessException, InvocationTargetException {
        Object[] arguments = arguments(parameterTypes, initargs);
        if (slot < 0) {
            throw new InstantiationException(null);
        }
        Object made;
        try {
            made = newInstance0(slot, arguments);
        } catch (Throwable t) {
            throw new InvocationTargetException(t);
        }
        return (T) made;
    }

    // The object that this constructor without parameters creates, whose exception goes on as
    // it is (`Class.newInstance`).
    @SuppressWarnings("unchecked")
    T create() throws InstantiationException {
        if (slot < 0) {
            throw new InstantiationException(null);
        }
        return (T) newInstance0(slot, new Object[0]);
    }

    public boolean equals(Object o) {
        if (!(o instanceof Constructor)) {
            return false;
        }
        Constructor<?> other = (Constructor<?>) o;
        return clazz == other.clazz && same(parameterTypes, other.parameterTypes);
    }

    public int hashCode() {
        return clazz.getName().hashCode();
    }

    public String toString() {
        return modifierText(modifiers & ~0x80) + typeName(clazz) + "(" + typeNames(parameterTypes) + ")";
    }

    // `c.getConstructor(types)`, or with `declared` `c.getDeclaredConstructor(types)`.
    static Constructor<?> find(Class<?> c, Class<?>[] types, boolean declared) throws NoSuchMethodException {
        Constructor<?>[] all = table();
        for (int i = 0; i < all.length; i++) {
            Constructor<?> k = all[i];
            if (k.clazz == c && (declared || (k.modifiers & 1) != 0) && same(k.parameterTypes, types)) {
                return k;
            }
        }
        throw new NoSuchMethodException(c.getName() + ".<init>(" + names(types) + ")" + MISSING);
    }

    static Constructor<?>[] list(Class<?> c, boolean declared) {
        Constructor<?>[] all = table();
        int n = 0;
        for (int i = 0; i < all.length; i++) {
            if (all[i].clazz == c && (declared || (all[i].modifiers & 1) != 0)) {
                n++;
            }
        }
        Constructor<?>[] out = new Constructor<?>[n];
        n = 0;
        for (int i = 0; i < all.length; i++) {
            if (all[i].clazz == c && (declared || (all[i].modifiers & 1) != 0)) {
                out[n] = all[i];
                n++;
            }
        }
        return out;
    }

    // `c.newInstance()`: the object that the constructor without parameters creates.
    static Object newInstanceOf(Class<?> c) throws InstantiationException, IllegalAccessException {
        try {
            return find(c, new Class<?>[0], true).create();
        } catch (NoSuchMethodException e) {
            throw new InstantiationException(c.getName() + MISSING);
        }
    }

    // `c.asSubclass(of)`, given whether `of` is assignable from `c`, which lowering decides.
    static Class<?> subclass(Class<?> c, boolean assignable) {
        if (!assignable) {
            throw new ClassCastException(c.toString());
        }
        return c;
    }

    private static Constructor<?>[] table() {
        if (table == null) {
            table = table0();
        }
        return table;
    }

    private static native Constructor<?>[] table0();

    private static native Object newInstance0(int slot, Object[] args);

    // What follows is shared by the three classes.

    static Class<?>[] copy(Class<?>[] types) {
        Class<?>[] out = new Class<?>[types.length];
        for (int i = 0; i < types.length; i++) {
            out[i] = types[i];
        }
        return out;
    }

    // Whether the parameter types are those asked for (`null` asks for none).
    static boolean same(Class<?>[] types, Class<?>[] asked) {
        int n = asked == null ? 0 : asked.length;
        if (types.length != n) {
            return false;
        }
        for (int i = 0; i < n; i++) {
            if (types[i] != asked[i]) {
                return false;
            }
        }
        return true;
    }

    // The names of the types asked for, as `NoSuchMethodException` gives them.
    static String names(Class<?>[] types) {
        String out = "";
        int n = types == null ? 0 : types.length;
        for (int i = 0; i < n; i++) {
            if (i > 0) {
                out = out + ",";
            }
            out = out + (types[i] == null ? "null" : types[i].getName());
        }
        return out;
    }

    static String typeName(Class<?> c) {
        if (c.isArray()) {
            return typeName(c.getComponentType()) + "[]";
        }
        return c.getName();
    }

    // `c.descriptorString()`: the class's descriptor, as a class file writes it. (Not reflection's,
    // but answered the same way.) A hidden class is not in a translation.
    static String descriptor(Class<?> c) {
        if (c.isArray()) {
            return c.getName().replace('.', '/');
        }
        if (c.isPrimitive()) {
            switch (c.getName()) {
                case "boolean": return "Z";
                case "byte": return "B";
                case "char": return "C";
                case "short": return "S";
                case "int": return "I";
                case "long": return "J";
                case "float": return "F";
                case "double": return "D";
                default: return "V";
            }
        }
        return "L" + c.getName().replace('.', '/') + ";";
    }

    static String typeNames(Class<?>[] types) {
        String out = "";
        for (int i = 0; i < types.length; i++) {
            if (i > 0) {
                out = out + ",";
            }
            out = out + typeName(types[i]);
        }
        return out;
    }

    // The modifiers as `Modifier.toString` writes them, with a space after them.
    static String modifierText(int m) {
        String out = "";
        String[] names = {"public", "protected", "private", "abstract", "static", "final", "transient",
            "volatile", "synchronized", "native"};
        int[] bits = {1, 4, 2, 0x400, 8, 0x10, 0x80, 0x40, 0x20, 0x100};
        for (int i = 0; i < bits.length; i++) {
            if ((m & bits[i]) != 0) {
                out = out + names[i] + " ";
            }
        }
        return out;
    }

    // The arguments of a call through the table, each as the box of its parameter's type, which
    // the call unboxes: a primitive argument may widen, as Java's reflection allows.
    static Object[] arguments(Class<?>[] types, Object[] args) {
        int n = args == null ? 0 : args.length;
        if (n != types.length) {
            throw new IllegalArgumentException("wrong number of arguments: " + n + " expected: " + types.length);
        }
        Object[] out = new Object[n];
        for (int i = 0; i < n; i++) {
            out[i] = argument(types[i], args[i]);
        }
        return out;
    }

    static Object argument(Class<?> type, Object value) {
        if (!type.isPrimitive()) {
            if (value != null && !type.isInstance(value)) {
                throw new IllegalArgumentException("argument type mismatch");
            }
            return value;
        }
        if (type == boolean.class && value instanceof Boolean) {
            return value;
        }
        if (type == char.class && value instanceof Character) {
            return value;
        }
        long l;
        if (value instanceof Byte) {
            l = (Byte) value;
        } else if (value instanceof Short) {
            l = (Short) value;
        } else if (value instanceof Character) {
            l = (Character) value;
        } else if (value instanceof Integer) {
            l = (Integer) value;
        } else if (value instanceof Long) {
            l = (Long) value;
        } else if (value instanceof Float && type == float.class || value instanceof Double && type == double.class) {
            return value;
        } else if (value instanceof Float && type == double.class) {
            return (double) (Float) value;
        } else {
            throw new IllegalArgumentException("argument type mismatch");
        }
        // An integral value widens to a wider type, and only to one.
        int rank = value instanceof Byte ? 1 : value instanceof Short || value instanceof Character ? 2
                : value instanceof Integer ? 3 : 4;
        if (type == short.class && rank == 1 && !(value instanceof Character)) {
            return (short) l;
        }
        if (type == byte.class && value instanceof Byte || type == short.class && value instanceof Short) {
            return value;
        }
        if (type == int.class && rank <= 3) {
            return (int) l;
        }
        if (type == long.class) {
            return l;
        }
        if (type == float.class) {
            return (float) l;
        }
        if (type == double.class) {
            return (double) l;
        }
        throw new IllegalArgumentException("argument type mismatch");
    }
}
