// `java.lang.reflect.Method`, `Field` and `Constructor` as the translator provides them, over the
// translation's reflection table: the members of the program's classes that reflection can reach,
// which the translator chose when translating (see `lower/reflection.rs`). `build.rs` prints the
// syntax tree of this file with `JavaTree`, and lowering gives its classes their real package and
// adds them, with the members a program uses, to the programs that use them, as it adds the
// standard exception classes (see `lower/exceptions.rs`). Each member has a slot in the table;
// the natives `tablePart`, `invokePart`, `getPart`, `setPart` and `newInstancePart` are written by
// lowering for each program, each for one part of the slots: the part's members, and a `switch` over
// its slots that calls the member of each.
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
    private final Class<?>[] exceptionTypes;
    private final int modifiers;
    private final int slot;

    Method(Class<?> owner, Class<?> clazz, String name, Class<?>[] parameterTypes, Class<?> returnType,
            Class<?>[] exceptionTypes, int modifiers, int slot) {
        this.owner = owner;
        this.clazz = clazz;
        this.name = name;
        this.parameterTypes = parameterTypes;
        this.returnType = returnType;
        this.exceptionTypes = exceptionTypes;
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

    public Class<?>[] getExceptionTypes() {
        return Constructor.copy(exceptionTypes);
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

    // Every member is accessible, as `canAccess` says: the table's members are shared by every
    // lookup, so there is no flag of one lookup's for `setAccessible` to set.
    @Deprecated
    public boolean isAccessible() {
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
        // A default method's access, `default`, then its other modifiers, as `Executable` writes them.
        String modifierText = isDefault()
                ? Constructor.modifierText(modifiers & 7) + "default " + Constructor.modifierText(modifiers & ~7 & ~0x80)
                : Constructor.modifierText(modifiers & ~0x80);
        return modifierText + Constructor.typeName(returnType) + " "
                + Constructor.typeName(clazz) + "." + name + "(" + Constructor.typeNames(parameterTypes) + ")"
                + Constructor.throwsText(exceptionTypes);
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

    // The table in the parts of its slots: part `p` holds slots `p`, `p + 32`, `p + 64` and so
    // on, in order, each written by lowering as its own array.
    private static Method[] table0() {
        Method[][] parts = {
            tablePart0(), tablePart1(), tablePart2(), tablePart3(),
            tablePart4(), tablePart5(), tablePart6(), tablePart7(),
            tablePart8(), tablePart9(), tablePart10(), tablePart11(),
            tablePart12(), tablePart13(), tablePart14(), tablePart15(),
            tablePart16(), tablePart17(), tablePart18(), tablePart19(),
            tablePart20(), tablePart21(), tablePart22(), tablePart23(),
            tablePart24(), tablePart25(), tablePart26(), tablePart27(),
            tablePart28(), tablePart29(), tablePart30(), tablePart31()
        };
        int length = 0;
        for (Method[] part : parts) {
            length += part.length;
        }
        Method[] table = new Method[length];
        for (int p = 0; p < 32; p++) {
            for (int i = 0; i < parts[p].length; i++) {
                table[i * 32 + p] = parts[p][i];
            }
        }
        return table;
    }

    private static Method[] tablePart0() { return tablePart(0); }
    private static Method[] tablePart1() { return tablePart(1); }
    private static Method[] tablePart2() { return tablePart(2); }
    private static Method[] tablePart3() { return tablePart(3); }
    private static Method[] tablePart4() { return tablePart(4); }
    private static Method[] tablePart5() { return tablePart(5); }
    private static Method[] tablePart6() { return tablePart(6); }
    private static Method[] tablePart7() { return tablePart(7); }
    private static Method[] tablePart8() { return tablePart(8); }
    private static Method[] tablePart9() { return tablePart(9); }
    private static Method[] tablePart10() { return tablePart(10); }
    private static Method[] tablePart11() { return tablePart(11); }
    private static Method[] tablePart12() { return tablePart(12); }
    private static Method[] tablePart13() { return tablePart(13); }
    private static Method[] tablePart14() { return tablePart(14); }
    private static Method[] tablePart15() { return tablePart(15); }
    private static Method[] tablePart16() { return tablePart(16); }
    private static Method[] tablePart17() { return tablePart(17); }
    private static Method[] tablePart18() { return tablePart(18); }
    private static Method[] tablePart19() { return tablePart(19); }
    private static Method[] tablePart20() { return tablePart(20); }
    private static Method[] tablePart21() { return tablePart(21); }
    private static Method[] tablePart22() { return tablePart(22); }
    private static Method[] tablePart23() { return tablePart(23); }
    private static Method[] tablePart24() { return tablePart(24); }
    private static Method[] tablePart25() { return tablePart(25); }
    private static Method[] tablePart26() { return tablePart(26); }
    private static Method[] tablePart27() { return tablePart(27); }
    private static Method[] tablePart28() { return tablePart(28); }
    private static Method[] tablePart29() { return tablePart(29); }
    private static Method[] tablePart30() { return tablePart(30); }
    private static Method[] tablePart31() { return tablePart(31); }

    private static native Method[] tablePart(int part);

    // The calls of the table's members, in 32 parts: slot `s` is in part `s & 31`, whose native
    // lowering writes as a `switch` over the slots of the part, so that no function of a big
    // program's translation holds them all.
    private static Object invoke0(int slot, Object obj, Object[] args) {
        return switch (slot & 31) {
            case 0 -> invokePart0(slot, obj, args);
            case 1 -> invokePart1(slot, obj, args);
            case 2 -> invokePart2(slot, obj, args);
            case 3 -> invokePart3(slot, obj, args);
            case 4 -> invokePart4(slot, obj, args);
            case 5 -> invokePart5(slot, obj, args);
            case 6 -> invokePart6(slot, obj, args);
            case 7 -> invokePart7(slot, obj, args);
            case 8 -> invokePart8(slot, obj, args);
            case 9 -> invokePart9(slot, obj, args);
            case 10 -> invokePart10(slot, obj, args);
            case 11 -> invokePart11(slot, obj, args);
            case 12 -> invokePart12(slot, obj, args);
            case 13 -> invokePart13(slot, obj, args);
            case 14 -> invokePart14(slot, obj, args);
            case 15 -> invokePart15(slot, obj, args);
            case 16 -> invokePart16(slot, obj, args);
            case 17 -> invokePart17(slot, obj, args);
            case 18 -> invokePart18(slot, obj, args);
            case 19 -> invokePart19(slot, obj, args);
            case 20 -> invokePart20(slot, obj, args);
            case 21 -> invokePart21(slot, obj, args);
            case 22 -> invokePart22(slot, obj, args);
            case 23 -> invokePart23(slot, obj, args);
            case 24 -> invokePart24(slot, obj, args);
            case 25 -> invokePart25(slot, obj, args);
            case 26 -> invokePart26(slot, obj, args);
            case 27 -> invokePart27(slot, obj, args);
            case 28 -> invokePart28(slot, obj, args);
            case 29 -> invokePart29(slot, obj, args);
            case 30 -> invokePart30(slot, obj, args);
            default -> invokePart31(slot, obj, args);
        };
    }
    private static Object invokePart0(int slot, Object obj, Object[] args) { return invokePart(0, slot, obj, args); }
    private static Object invokePart1(int slot, Object obj, Object[] args) { return invokePart(1, slot, obj, args); }
    private static Object invokePart2(int slot, Object obj, Object[] args) { return invokePart(2, slot, obj, args); }
    private static Object invokePart3(int slot, Object obj, Object[] args) { return invokePart(3, slot, obj, args); }
    private static Object invokePart4(int slot, Object obj, Object[] args) { return invokePart(4, slot, obj, args); }
    private static Object invokePart5(int slot, Object obj, Object[] args) { return invokePart(5, slot, obj, args); }
    private static Object invokePart6(int slot, Object obj, Object[] args) { return invokePart(6, slot, obj, args); }
    private static Object invokePart7(int slot, Object obj, Object[] args) { return invokePart(7, slot, obj, args); }
    private static Object invokePart8(int slot, Object obj, Object[] args) { return invokePart(8, slot, obj, args); }
    private static Object invokePart9(int slot, Object obj, Object[] args) { return invokePart(9, slot, obj, args); }
    private static Object invokePart10(int slot, Object obj, Object[] args) { return invokePart(10, slot, obj, args); }
    private static Object invokePart11(int slot, Object obj, Object[] args) { return invokePart(11, slot, obj, args); }
    private static Object invokePart12(int slot, Object obj, Object[] args) { return invokePart(12, slot, obj, args); }
    private static Object invokePart13(int slot, Object obj, Object[] args) { return invokePart(13, slot, obj, args); }
    private static Object invokePart14(int slot, Object obj, Object[] args) { return invokePart(14, slot, obj, args); }
    private static Object invokePart15(int slot, Object obj, Object[] args) { return invokePart(15, slot, obj, args); }
    private static Object invokePart16(int slot, Object obj, Object[] args) { return invokePart(16, slot, obj, args); }
    private static Object invokePart17(int slot, Object obj, Object[] args) { return invokePart(17, slot, obj, args); }
    private static Object invokePart18(int slot, Object obj, Object[] args) { return invokePart(18, slot, obj, args); }
    private static Object invokePart19(int slot, Object obj, Object[] args) { return invokePart(19, slot, obj, args); }
    private static Object invokePart20(int slot, Object obj, Object[] args) { return invokePart(20, slot, obj, args); }
    private static Object invokePart21(int slot, Object obj, Object[] args) { return invokePart(21, slot, obj, args); }
    private static Object invokePart22(int slot, Object obj, Object[] args) { return invokePart(22, slot, obj, args); }
    private static Object invokePart23(int slot, Object obj, Object[] args) { return invokePart(23, slot, obj, args); }
    private static Object invokePart24(int slot, Object obj, Object[] args) { return invokePart(24, slot, obj, args); }
    private static Object invokePart25(int slot, Object obj, Object[] args) { return invokePart(25, slot, obj, args); }
    private static Object invokePart26(int slot, Object obj, Object[] args) { return invokePart(26, slot, obj, args); }
    private static Object invokePart27(int slot, Object obj, Object[] args) { return invokePart(27, slot, obj, args); }
    private static Object invokePart28(int slot, Object obj, Object[] args) { return invokePart(28, slot, obj, args); }
    private static Object invokePart29(int slot, Object obj, Object[] args) { return invokePart(29, slot, obj, args); }
    private static Object invokePart30(int slot, Object obj, Object[] args) { return invokePart(30, slot, obj, args); }
    private static Object invokePart31(int slot, Object obj, Object[] args) { return invokePart(31, slot, obj, args); }

    private static native Object invokePart(int part, int slot, Object obj, Object[] args);
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

    // Every member is accessible, as `canAccess` says: the table's members are shared by every
    // lookup, so there is no flag of one lookup's for `setAccessible` to set.
    @Deprecated
    public boolean isAccessible() {
        return true;
    }

    public boolean isEnumConstant() {
        return (modifiers & 0x4000) != 0;
    }

    public boolean isSynthetic() {
        return (modifiers & 0x1000) != 0;
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

    // `Unsafe.objectFieldOffset(f)` used as a number (`lower/handles.rs`): a number of the
    // field's own, the same for every `Field` of it, 16 apart, which tells nothing of where the
    // object keeps it, so that code which checks the layout before reading memory at an offset
    // takes its other way. `sun` (`sun.misc.Unsafe`'s, which refuses a record's field) is not
    // told apart yet: a record's field gets its number too.
    static long objectFieldOffset(Field f, boolean sun) {
        if (f == null) {
            throw new NullPointerException();
        }
        if ((f.modifiers & 8) != 0) {
            throw new IllegalArgumentException();
        }
        Field[] all = table();
        int i = 0;
        while (all[i].clazz != f.clazz || !all[i].name.equals(f.name)) {
            i++;
        }
        return 16L * (i + 1);
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

    // The table in the parts of its slots: part `p` holds slots `p`, `p + 32`, `p + 64` and so
    // on, in order, each written by lowering as its own array.
    private static Field[] table0() {
        Field[][] parts = {
            tablePart0(), tablePart1(), tablePart2(), tablePart3(),
            tablePart4(), tablePart5(), tablePart6(), tablePart7(),
            tablePart8(), tablePart9(), tablePart10(), tablePart11(),
            tablePart12(), tablePart13(), tablePart14(), tablePart15(),
            tablePart16(), tablePart17(), tablePart18(), tablePart19(),
            tablePart20(), tablePart21(), tablePart22(), tablePart23(),
            tablePart24(), tablePart25(), tablePart26(), tablePart27(),
            tablePart28(), tablePart29(), tablePart30(), tablePart31()
        };
        int length = 0;
        for (Field[] part : parts) {
            length += part.length;
        }
        Field[] table = new Field[length];
        for (int p = 0; p < 32; p++) {
            for (int i = 0; i < parts[p].length; i++) {
                table[i * 32 + p] = parts[p][i];
            }
        }
        return table;
    }

    private static Field[] tablePart0() { return tablePart(0); }
    private static Field[] tablePart1() { return tablePart(1); }
    private static Field[] tablePart2() { return tablePart(2); }
    private static Field[] tablePart3() { return tablePart(3); }
    private static Field[] tablePart4() { return tablePart(4); }
    private static Field[] tablePart5() { return tablePart(5); }
    private static Field[] tablePart6() { return tablePart(6); }
    private static Field[] tablePart7() { return tablePart(7); }
    private static Field[] tablePart8() { return tablePart(8); }
    private static Field[] tablePart9() { return tablePart(9); }
    private static Field[] tablePart10() { return tablePart(10); }
    private static Field[] tablePart11() { return tablePart(11); }
    private static Field[] tablePart12() { return tablePart(12); }
    private static Field[] tablePart13() { return tablePart(13); }
    private static Field[] tablePart14() { return tablePart(14); }
    private static Field[] tablePart15() { return tablePart(15); }
    private static Field[] tablePart16() { return tablePart(16); }
    private static Field[] tablePart17() { return tablePart(17); }
    private static Field[] tablePart18() { return tablePart(18); }
    private static Field[] tablePart19() { return tablePart(19); }
    private static Field[] tablePart20() { return tablePart(20); }
    private static Field[] tablePart21() { return tablePart(21); }
    private static Field[] tablePart22() { return tablePart(22); }
    private static Field[] tablePart23() { return tablePart(23); }
    private static Field[] tablePart24() { return tablePart(24); }
    private static Field[] tablePart25() { return tablePart(25); }
    private static Field[] tablePart26() { return tablePart(26); }
    private static Field[] tablePart27() { return tablePart(27); }
    private static Field[] tablePart28() { return tablePart(28); }
    private static Field[] tablePart29() { return tablePart(29); }
    private static Field[] tablePart30() { return tablePart(30); }
    private static Field[] tablePart31() { return tablePart(31); }

    private static native Field[] tablePart(int part);

    // The reads of the table's fields, in 32 parts: slot `s` is in part `s & 31`, whose native
    // lowering writes as a `switch` over the slots of the part, so that no function of a big
    // program's translation holds them all.
    private static Object get0(int slot, Object obj) {
        return switch (slot & 31) {
            case 0 -> getPart0(slot, obj);
            case 1 -> getPart1(slot, obj);
            case 2 -> getPart2(slot, obj);
            case 3 -> getPart3(slot, obj);
            case 4 -> getPart4(slot, obj);
            case 5 -> getPart5(slot, obj);
            case 6 -> getPart6(slot, obj);
            case 7 -> getPart7(slot, obj);
            case 8 -> getPart8(slot, obj);
            case 9 -> getPart9(slot, obj);
            case 10 -> getPart10(slot, obj);
            case 11 -> getPart11(slot, obj);
            case 12 -> getPart12(slot, obj);
            case 13 -> getPart13(slot, obj);
            case 14 -> getPart14(slot, obj);
            case 15 -> getPart15(slot, obj);
            case 16 -> getPart16(slot, obj);
            case 17 -> getPart17(slot, obj);
            case 18 -> getPart18(slot, obj);
            case 19 -> getPart19(slot, obj);
            case 20 -> getPart20(slot, obj);
            case 21 -> getPart21(slot, obj);
            case 22 -> getPart22(slot, obj);
            case 23 -> getPart23(slot, obj);
            case 24 -> getPart24(slot, obj);
            case 25 -> getPart25(slot, obj);
            case 26 -> getPart26(slot, obj);
            case 27 -> getPart27(slot, obj);
            case 28 -> getPart28(slot, obj);
            case 29 -> getPart29(slot, obj);
            case 30 -> getPart30(slot, obj);
            default -> getPart31(slot, obj);
        };
    }
    private static Object getPart0(int slot, Object obj) { return getPart(0, slot, obj); }
    private static Object getPart1(int slot, Object obj) { return getPart(1, slot, obj); }
    private static Object getPart2(int slot, Object obj) { return getPart(2, slot, obj); }
    private static Object getPart3(int slot, Object obj) { return getPart(3, slot, obj); }
    private static Object getPart4(int slot, Object obj) { return getPart(4, slot, obj); }
    private static Object getPart5(int slot, Object obj) { return getPart(5, slot, obj); }
    private static Object getPart6(int slot, Object obj) { return getPart(6, slot, obj); }
    private static Object getPart7(int slot, Object obj) { return getPart(7, slot, obj); }
    private static Object getPart8(int slot, Object obj) { return getPart(8, slot, obj); }
    private static Object getPart9(int slot, Object obj) { return getPart(9, slot, obj); }
    private static Object getPart10(int slot, Object obj) { return getPart(10, slot, obj); }
    private static Object getPart11(int slot, Object obj) { return getPart(11, slot, obj); }
    private static Object getPart12(int slot, Object obj) { return getPart(12, slot, obj); }
    private static Object getPart13(int slot, Object obj) { return getPart(13, slot, obj); }
    private static Object getPart14(int slot, Object obj) { return getPart(14, slot, obj); }
    private static Object getPart15(int slot, Object obj) { return getPart(15, slot, obj); }
    private static Object getPart16(int slot, Object obj) { return getPart(16, slot, obj); }
    private static Object getPart17(int slot, Object obj) { return getPart(17, slot, obj); }
    private static Object getPart18(int slot, Object obj) { return getPart(18, slot, obj); }
    private static Object getPart19(int slot, Object obj) { return getPart(19, slot, obj); }
    private static Object getPart20(int slot, Object obj) { return getPart(20, slot, obj); }
    private static Object getPart21(int slot, Object obj) { return getPart(21, slot, obj); }
    private static Object getPart22(int slot, Object obj) { return getPart(22, slot, obj); }
    private static Object getPart23(int slot, Object obj) { return getPart(23, slot, obj); }
    private static Object getPart24(int slot, Object obj) { return getPart(24, slot, obj); }
    private static Object getPart25(int slot, Object obj) { return getPart(25, slot, obj); }
    private static Object getPart26(int slot, Object obj) { return getPart(26, slot, obj); }
    private static Object getPart27(int slot, Object obj) { return getPart(27, slot, obj); }
    private static Object getPart28(int slot, Object obj) { return getPart(28, slot, obj); }
    private static Object getPart29(int slot, Object obj) { return getPart(29, slot, obj); }
    private static Object getPart30(int slot, Object obj) { return getPart(30, slot, obj); }
    private static Object getPart31(int slot, Object obj) { return getPart(31, slot, obj); }

    private static native Object getPart(int part, int slot, Object obj);

    // The writes, in the same parts. Each gives the value written: lowering writes it as an
    // expression, whose value has a type.
    private static Object set0(int slot, Object obj, Object value) {
        return switch (slot & 31) {
            case 0 -> setPart0(slot, obj, value);
            case 1 -> setPart1(slot, obj, value);
            case 2 -> setPart2(slot, obj, value);
            case 3 -> setPart3(slot, obj, value);
            case 4 -> setPart4(slot, obj, value);
            case 5 -> setPart5(slot, obj, value);
            case 6 -> setPart6(slot, obj, value);
            case 7 -> setPart7(slot, obj, value);
            case 8 -> setPart8(slot, obj, value);
            case 9 -> setPart9(slot, obj, value);
            case 10 -> setPart10(slot, obj, value);
            case 11 -> setPart11(slot, obj, value);
            case 12 -> setPart12(slot, obj, value);
            case 13 -> setPart13(slot, obj, value);
            case 14 -> setPart14(slot, obj, value);
            case 15 -> setPart15(slot, obj, value);
            case 16 -> setPart16(slot, obj, value);
            case 17 -> setPart17(slot, obj, value);
            case 18 -> setPart18(slot, obj, value);
            case 19 -> setPart19(slot, obj, value);
            case 20 -> setPart20(slot, obj, value);
            case 21 -> setPart21(slot, obj, value);
            case 22 -> setPart22(slot, obj, value);
            case 23 -> setPart23(slot, obj, value);
            case 24 -> setPart24(slot, obj, value);
            case 25 -> setPart25(slot, obj, value);
            case 26 -> setPart26(slot, obj, value);
            case 27 -> setPart27(slot, obj, value);
            case 28 -> setPart28(slot, obj, value);
            case 29 -> setPart29(slot, obj, value);
            case 30 -> setPart30(slot, obj, value);
            default -> setPart31(slot, obj, value);
        };
    }
    private static Object setPart0(int slot, Object obj, Object value) { return setPart(0, slot, obj, value); }
    private static Object setPart1(int slot, Object obj, Object value) { return setPart(1, slot, obj, value); }
    private static Object setPart2(int slot, Object obj, Object value) { return setPart(2, slot, obj, value); }
    private static Object setPart3(int slot, Object obj, Object value) { return setPart(3, slot, obj, value); }
    private static Object setPart4(int slot, Object obj, Object value) { return setPart(4, slot, obj, value); }
    private static Object setPart5(int slot, Object obj, Object value) { return setPart(5, slot, obj, value); }
    private static Object setPart6(int slot, Object obj, Object value) { return setPart(6, slot, obj, value); }
    private static Object setPart7(int slot, Object obj, Object value) { return setPart(7, slot, obj, value); }
    private static Object setPart8(int slot, Object obj, Object value) { return setPart(8, slot, obj, value); }
    private static Object setPart9(int slot, Object obj, Object value) { return setPart(9, slot, obj, value); }
    private static Object setPart10(int slot, Object obj, Object value) { return setPart(10, slot, obj, value); }
    private static Object setPart11(int slot, Object obj, Object value) { return setPart(11, slot, obj, value); }
    private static Object setPart12(int slot, Object obj, Object value) { return setPart(12, slot, obj, value); }
    private static Object setPart13(int slot, Object obj, Object value) { return setPart(13, slot, obj, value); }
    private static Object setPart14(int slot, Object obj, Object value) { return setPart(14, slot, obj, value); }
    private static Object setPart15(int slot, Object obj, Object value) { return setPart(15, slot, obj, value); }
    private static Object setPart16(int slot, Object obj, Object value) { return setPart(16, slot, obj, value); }
    private static Object setPart17(int slot, Object obj, Object value) { return setPart(17, slot, obj, value); }
    private static Object setPart18(int slot, Object obj, Object value) { return setPart(18, slot, obj, value); }
    private static Object setPart19(int slot, Object obj, Object value) { return setPart(19, slot, obj, value); }
    private static Object setPart20(int slot, Object obj, Object value) { return setPart(20, slot, obj, value); }
    private static Object setPart21(int slot, Object obj, Object value) { return setPart(21, slot, obj, value); }
    private static Object setPart22(int slot, Object obj, Object value) { return setPart(22, slot, obj, value); }
    private static Object setPart23(int slot, Object obj, Object value) { return setPart(23, slot, obj, value); }
    private static Object setPart24(int slot, Object obj, Object value) { return setPart(24, slot, obj, value); }
    private static Object setPart25(int slot, Object obj, Object value) { return setPart(25, slot, obj, value); }
    private static Object setPart26(int slot, Object obj, Object value) { return setPart(26, slot, obj, value); }
    private static Object setPart27(int slot, Object obj, Object value) { return setPart(27, slot, obj, value); }
    private static Object setPart28(int slot, Object obj, Object value) { return setPart(28, slot, obj, value); }
    private static Object setPart29(int slot, Object obj, Object value) { return setPart(29, slot, obj, value); }
    private static Object setPart30(int slot, Object obj, Object value) { return setPart(30, slot, obj, value); }
    private static Object setPart31(int slot, Object obj, Object value) { return setPart(31, slot, obj, value); }

    private static native Object setPart(int part, int slot, Object obj, Object value);
}

final class Constructor<T> {
    // The text that the exceptions of a lookup by a name not in the table end with.
    static final String MISSING = ", which is not in the translation";

    private static Constructor<?>[] table;

    private final Class<T> clazz;
    private final Class<?>[] parameterTypes;
    private final Class<?>[] exceptionTypes;
    private final int modifiers;
    private final int slot;

    Constructor(Class<T> clazz, Class<?>[] parameterTypes, Class<?>[] exceptionTypes, int modifiers, int slot) {
        this.clazz = clazz;
        this.parameterTypes = parameterTypes;
        this.exceptionTypes = exceptionTypes;
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

    public Class<?>[] getExceptionTypes() {
        return copy(exceptionTypes);
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

    // Every member is accessible, as `canAccess` says: the table's members are shared by every
    // lookup, so there is no flag of one lookup's for `setAccessible` to set.
    @Deprecated
    public boolean isAccessible() {
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
        return modifierText(modifiers & ~0x80) + typeName(clazz) + "(" + typeNames(parameterTypes) + ")"
                + throwsText(exceptionTypes);
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

    // The table in the parts of its slots: part `p` holds slots `p`, `p + 32`, `p + 64` and so
    // on, in order, each written by lowering as its own array.
    private static Constructor<?>[] table0() {
        Constructor<?>[][] parts = {
            tablePart0(), tablePart1(), tablePart2(), tablePart3(),
            tablePart4(), tablePart5(), tablePart6(), tablePart7(),
            tablePart8(), tablePart9(), tablePart10(), tablePart11(),
            tablePart12(), tablePart13(), tablePart14(), tablePart15(),
            tablePart16(), tablePart17(), tablePart18(), tablePart19(),
            tablePart20(), tablePart21(), tablePart22(), tablePart23(),
            tablePart24(), tablePart25(), tablePart26(), tablePart27(),
            tablePart28(), tablePart29(), tablePart30(), tablePart31()
        };
        int length = 0;
        for (Constructor<?>[] part : parts) {
            length += part.length;
        }
        Constructor<?>[] table = new Constructor<?>[length];
        for (int p = 0; p < 32; p++) {
            for (int i = 0; i < parts[p].length; i++) {
                table[i * 32 + p] = parts[p][i];
            }
        }
        return table;
    }

    private static Constructor<?>[] tablePart0() { return tablePart(0); }
    private static Constructor<?>[] tablePart1() { return tablePart(1); }
    private static Constructor<?>[] tablePart2() { return tablePart(2); }
    private static Constructor<?>[] tablePart3() { return tablePart(3); }
    private static Constructor<?>[] tablePart4() { return tablePart(4); }
    private static Constructor<?>[] tablePart5() { return tablePart(5); }
    private static Constructor<?>[] tablePart6() { return tablePart(6); }
    private static Constructor<?>[] tablePart7() { return tablePart(7); }
    private static Constructor<?>[] tablePart8() { return tablePart(8); }
    private static Constructor<?>[] tablePart9() { return tablePart(9); }
    private static Constructor<?>[] tablePart10() { return tablePart(10); }
    private static Constructor<?>[] tablePart11() { return tablePart(11); }
    private static Constructor<?>[] tablePart12() { return tablePart(12); }
    private static Constructor<?>[] tablePart13() { return tablePart(13); }
    private static Constructor<?>[] tablePart14() { return tablePart(14); }
    private static Constructor<?>[] tablePart15() { return tablePart(15); }
    private static Constructor<?>[] tablePart16() { return tablePart(16); }
    private static Constructor<?>[] tablePart17() { return tablePart(17); }
    private static Constructor<?>[] tablePart18() { return tablePart(18); }
    private static Constructor<?>[] tablePart19() { return tablePart(19); }
    private static Constructor<?>[] tablePart20() { return tablePart(20); }
    private static Constructor<?>[] tablePart21() { return tablePart(21); }
    private static Constructor<?>[] tablePart22() { return tablePart(22); }
    private static Constructor<?>[] tablePart23() { return tablePart(23); }
    private static Constructor<?>[] tablePart24() { return tablePart(24); }
    private static Constructor<?>[] tablePart25() { return tablePart(25); }
    private static Constructor<?>[] tablePart26() { return tablePart(26); }
    private static Constructor<?>[] tablePart27() { return tablePart(27); }
    private static Constructor<?>[] tablePart28() { return tablePart(28); }
    private static Constructor<?>[] tablePart29() { return tablePart(29); }
    private static Constructor<?>[] tablePart30() { return tablePart(30); }
    private static Constructor<?>[] tablePart31() { return tablePart(31); }

    private static native Constructor<?>[] tablePart(int part);

    // The constructions of the table's constructors, in 32 parts: slot `s` is in part `s & 31`, whose native
    // lowering writes as a `switch` over the slots of the part, so that no function of a big
    // program's translation holds them all.
    private static Object newInstance0(int slot, Object[] args) {
        return switch (slot & 31) {
            case 0 -> newInstancePart0(slot, args);
            case 1 -> newInstancePart1(slot, args);
            case 2 -> newInstancePart2(slot, args);
            case 3 -> newInstancePart3(slot, args);
            case 4 -> newInstancePart4(slot, args);
            case 5 -> newInstancePart5(slot, args);
            case 6 -> newInstancePart6(slot, args);
            case 7 -> newInstancePart7(slot, args);
            case 8 -> newInstancePart8(slot, args);
            case 9 -> newInstancePart9(slot, args);
            case 10 -> newInstancePart10(slot, args);
            case 11 -> newInstancePart11(slot, args);
            case 12 -> newInstancePart12(slot, args);
            case 13 -> newInstancePart13(slot, args);
            case 14 -> newInstancePart14(slot, args);
            case 15 -> newInstancePart15(slot, args);
            case 16 -> newInstancePart16(slot, args);
            case 17 -> newInstancePart17(slot, args);
            case 18 -> newInstancePart18(slot, args);
            case 19 -> newInstancePart19(slot, args);
            case 20 -> newInstancePart20(slot, args);
            case 21 -> newInstancePart21(slot, args);
            case 22 -> newInstancePart22(slot, args);
            case 23 -> newInstancePart23(slot, args);
            case 24 -> newInstancePart24(slot, args);
            case 25 -> newInstancePart25(slot, args);
            case 26 -> newInstancePart26(slot, args);
            case 27 -> newInstancePart27(slot, args);
            case 28 -> newInstancePart28(slot, args);
            case 29 -> newInstancePart29(slot, args);
            case 30 -> newInstancePart30(slot, args);
            default -> newInstancePart31(slot, args);
        };
    }
    private static Object newInstancePart0(int slot, Object[] args) { return newInstancePart(0, slot, args); }
    private static Object newInstancePart1(int slot, Object[] args) { return newInstancePart(1, slot, args); }
    private static Object newInstancePart2(int slot, Object[] args) { return newInstancePart(2, slot, args); }
    private static Object newInstancePart3(int slot, Object[] args) { return newInstancePart(3, slot, args); }
    private static Object newInstancePart4(int slot, Object[] args) { return newInstancePart(4, slot, args); }
    private static Object newInstancePart5(int slot, Object[] args) { return newInstancePart(5, slot, args); }
    private static Object newInstancePart6(int slot, Object[] args) { return newInstancePart(6, slot, args); }
    private static Object newInstancePart7(int slot, Object[] args) { return newInstancePart(7, slot, args); }
    private static Object newInstancePart8(int slot, Object[] args) { return newInstancePart(8, slot, args); }
    private static Object newInstancePart9(int slot, Object[] args) { return newInstancePart(9, slot, args); }
    private static Object newInstancePart10(int slot, Object[] args) { return newInstancePart(10, slot, args); }
    private static Object newInstancePart11(int slot, Object[] args) { return newInstancePart(11, slot, args); }
    private static Object newInstancePart12(int slot, Object[] args) { return newInstancePart(12, slot, args); }
    private static Object newInstancePart13(int slot, Object[] args) { return newInstancePart(13, slot, args); }
    private static Object newInstancePart14(int slot, Object[] args) { return newInstancePart(14, slot, args); }
    private static Object newInstancePart15(int slot, Object[] args) { return newInstancePart(15, slot, args); }
    private static Object newInstancePart16(int slot, Object[] args) { return newInstancePart(16, slot, args); }
    private static Object newInstancePart17(int slot, Object[] args) { return newInstancePart(17, slot, args); }
    private static Object newInstancePart18(int slot, Object[] args) { return newInstancePart(18, slot, args); }
    private static Object newInstancePart19(int slot, Object[] args) { return newInstancePart(19, slot, args); }
    private static Object newInstancePart20(int slot, Object[] args) { return newInstancePart(20, slot, args); }
    private static Object newInstancePart21(int slot, Object[] args) { return newInstancePart(21, slot, args); }
    private static Object newInstancePart22(int slot, Object[] args) { return newInstancePart(22, slot, args); }
    private static Object newInstancePart23(int slot, Object[] args) { return newInstancePart(23, slot, args); }
    private static Object newInstancePart24(int slot, Object[] args) { return newInstancePart(24, slot, args); }
    private static Object newInstancePart25(int slot, Object[] args) { return newInstancePart(25, slot, args); }
    private static Object newInstancePart26(int slot, Object[] args) { return newInstancePart(26, slot, args); }
    private static Object newInstancePart27(int slot, Object[] args) { return newInstancePart(27, slot, args); }
    private static Object newInstancePart28(int slot, Object[] args) { return newInstancePart(28, slot, args); }
    private static Object newInstancePart29(int slot, Object[] args) { return newInstancePart(29, slot, args); }
    private static Object newInstancePart30(int slot, Object[] args) { return newInstancePart(30, slot, args); }
    private static Object newInstancePart31(int slot, Object[] args) { return newInstancePart(31, slot, args); }

    private static native Object newInstancePart(int part, int slot, Object[] args);

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

    // The `throws` clause of a member's `toString`, empty when it declares no exception.
    static String throwsText(Class<?>[] types) {
        return types.length == 0 ? "" : " throws " + typeNames(types);
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
