// Enum keys looked up as `Object`s, as `java.util.EnumMap.get` does (the Minecraft server's item
// entities look up their item's components this way): the key's class, from `getClass()`, is
// compared with the map's key type and, for a constant with a body, with its superclass, then the
// key is cast to `Enum` for its ordinal. The program has 200 more classes held as `Object`s, so
// `getClass()` finds the key's class among all of them.
public class EnumKeyLookups {
    enum Color { RED, GREEN, BLUE, CYAN, MAGENTA, YELLOW, BLACK, WHITE }
    enum Shape { CIRCLE, SQUARE, TRIANGLE, HEXAGON }
    enum Size { SMALL, MEDIUM, LARGE }
    enum Op {
        ADD { int apply(int a, int b) { return a + b; } },
        SUB { int apply(int a, int b) { return a - b; } },
        MUL { int apply(int a, int b) { return a * b; } };
        abstract int apply(int a, int b);
    }

    static final class F000 { }
    static final class F001 { }
    static final class F002 { }
    static final class F003 { }
    static final class F004 { }
    static final class F005 { }
    static final class F006 { }
    static final class F007 { }
    static final class F008 { }
    static final class F009 { }
    static final class F010 { }
    static final class F011 { }
    static final class F012 { }
    static final class F013 { }
    static final class F014 { }
    static final class F015 { }
    static final class F016 { }
    static final class F017 { }
    static final class F018 { }
    static final class F019 { }
    static final class F020 { }
    static final class F021 { }
    static final class F022 { }
    static final class F023 { }
    static final class F024 { }
    static final class F025 { }
    static final class F026 { }
    static final class F027 { }
    static final class F028 { }
    static final class F029 { }
    static final class F030 { }
    static final class F031 { }
    static final class F032 { }
    static final class F033 { }
    static final class F034 { }
    static final class F035 { }
    static final class F036 { }
    static final class F037 { }
    static final class F038 { }
    static final class F039 { }
    static final class F040 { }
    static final class F041 { }
    static final class F042 { }
    static final class F043 { }
    static final class F044 { }
    static final class F045 { }
    static final class F046 { }
    static final class F047 { }
    static final class F048 { }
    static final class F049 { }
    static final class F050 { }
    static final class F051 { }
    static final class F052 { }
    static final class F053 { }
    static final class F054 { }
    static final class F055 { }
    static final class F056 { }
    static final class F057 { }
    static final class F058 { }
    static final class F059 { }
    static final class F060 { }
    static final class F061 { }
    static final class F062 { }
    static final class F063 { }
    static final class F064 { }
    static final class F065 { }
    static final class F066 { }
    static final class F067 { }
    static final class F068 { }
    static final class F069 { }
    static final class F070 { }
    static final class F071 { }
    static final class F072 { }
    static final class F073 { }
    static final class F074 { }
    static final class F075 { }
    static final class F076 { }
    static final class F077 { }
    static final class F078 { }
    static final class F079 { }
    static final class F080 { }
    static final class F081 { }
    static final class F082 { }
    static final class F083 { }
    static final class F084 { }
    static final class F085 { }
    static final class F086 { }
    static final class F087 { }
    static final class F088 { }
    static final class F089 { }
    static final class F090 { }
    static final class F091 { }
    static final class F092 { }
    static final class F093 { }
    static final class F094 { }
    static final class F095 { }
    static final class F096 { }
    static final class F097 { }
    static final class F098 { }
    static final class F099 { }
    static final class F100 { }
    static final class F101 { }
    static final class F102 { }
    static final class F103 { }
    static final class F104 { }
    static final class F105 { }
    static final class F106 { }
    static final class F107 { }
    static final class F108 { }
    static final class F109 { }
    static final class F110 { }
    static final class F111 { }
    static final class F112 { }
    static final class F113 { }
    static final class F114 { }
    static final class F115 { }
    static final class F116 { }
    static final class F117 { }
    static final class F118 { }
    static final class F119 { }
    static final class F120 { }
    static final class F121 { }
    static final class F122 { }
    static final class F123 { }
    static final class F124 { }
    static final class F125 { }
    static final class F126 { }
    static final class F127 { }
    static final class F128 { }
    static final class F129 { }
    static final class F130 { }
    static final class F131 { }
    static final class F132 { }
    static final class F133 { }
    static final class F134 { }
    static final class F135 { }
    static final class F136 { }
    static final class F137 { }
    static final class F138 { }
    static final class F139 { }
    static final class F140 { }
    static final class F141 { }
    static final class F142 { }
    static final class F143 { }
    static final class F144 { }
    static final class F145 { }
    static final class F146 { }
    static final class F147 { }
    static final class F148 { }
    static final class F149 { }
    static final class F150 { }
    static final class F151 { }
    static final class F152 { }
    static final class F153 { }
    static final class F154 { }
    static final class F155 { }
    static final class F156 { }
    static final class F157 { }
    static final class F158 { }
    static final class F159 { }
    static final class F160 { }
    static final class F161 { }
    static final class F162 { }
    static final class F163 { }
    static final class F164 { }
    static final class F165 { }
    static final class F166 { }
    static final class F167 { }
    static final class F168 { }
    static final class F169 { }
    static final class F170 { }
    static final class F171 { }
    static final class F172 { }
    static final class F173 { }
    static final class F174 { }
    static final class F175 { }
    static final class F176 { }
    static final class F177 { }
    static final class F178 { }
    static final class F179 { }
    static final class F180 { }
    static final class F181 { }
    static final class F182 { }
    static final class F183 { }
    static final class F184 { }
    static final class F185 { }
    static final class F186 { }
    static final class F187 { }
    static final class F188 { }
    static final class F189 { }
    static final class F190 { }
    static final class F191 { }
    static final class F192 { }
    static final class F193 { }
    static final class F194 { }
    static final class F195 { }
    static final class F196 { }
    static final class F197 { }
    static final class F198 { }
    static final class F199 { }

    // `java.util.EnumMap`'s lookup: the values by ordinal, a null value held as a mark.
    static final class EnumTable<K extends Enum<K>> {
        static final Object NULL = new Object();
        final Class<K> keyType;
        final Object[] vals;

        EnumTable(Class<K> keyType, int size) {
            this.keyType = keyType;
            this.vals = new Object[size];
        }

        void put(K key, Object value) {
            vals[key.ordinal()] = value == null ? NULL : value;
        }

        Object get(Object key) {
            return isValidKey(key) ? unmaskNull(vals[((Enum<?>) key).ordinal()]) : null;
        }

        boolean isValidKey(Object key) {
            if (key == null) {
                return false;
            }
            Class<?> keyClass = key.getClass();
            return keyClass == keyType || keyClass.getSuperclass() == keyType;
        }

        static Object unmaskNull(Object value) {
            return value == NULL ? null : value;
        }
    }

    static long run(EnumTable<?>[] tables, Object[] keys, int rounds) {
        long sum = 0;
        for (int r = 0; r < rounds; r++) {
            for (int i = 0; i < keys.length; i++) {
                Object value = tables[i % tables.length].get(keys[i]);
                if (value instanceof Integer n) {
                    sum += n;
                } else if (value == null) {
                    sum += 1;
                }
            }
        }
        return sum;
    }

    public static void main(String[] args) {
        EnumTable<Color> colors = new EnumTable<>(Color.class, Color.values().length);
        for (Color c : Color.values()) {
            colors.put(c, c.ordinal() % 3 == 0 ? null : c.ordinal() * 7);
        }
        EnumTable<Shape> shapes = new EnumTable<>(Shape.class, Shape.values().length);
        for (Shape s : Shape.values()) {
            shapes.put(s, s.ordinal() + 100);
        }
        EnumTable<Size> sizes = new EnumTable<>(Size.class, Size.values().length);
        for (Size s : Size.values()) {
            sizes.put(s, s.ordinal() + 1000);
        }
        EnumTable<Op> ops = new EnumTable<>(Op.class, Op.values().length);
        for (Op o : Op.values()) {
            ops.put(o, o.apply(6, 3));
        }
        EnumTable<?>[] tables = { colors, shapes, sizes, ops };
        Object[] others = { new F000(), new F001(), new F002(), new F003(), new F004(), new F005(), new F006(), new F007(), new F008(), new F009(), new F010(), new F011(), new F012(), new F013(), new F014(), new F015(), new F016(), new F017(), new F018(), new F019(), new F020(), new F021(), new F022(), new F023(), new F024(), new F025(), new F026(), new F027(), new F028(), new F029(), new F030(), new F031(), new F032(), new F033(), new F034(), new F035(), new F036(), new F037(), new F038(), new F039(), new F040(), new F041(), new F042(), new F043(), new F044(), new F045(), new F046(), new F047(), new F048(), new F049(), new F050(), new F051(), new F052(), new F053(), new F054(), new F055(), new F056(), new F057(), new F058(), new F059(), new F060(), new F061(), new F062(), new F063(), new F064(), new F065(), new F066(), new F067(), new F068(), new F069(), new F070(), new F071(), new F072(), new F073(), new F074(), new F075(), new F076(), new F077(), new F078(), new F079(), new F080(), new F081(), new F082(), new F083(), new F084(), new F085(), new F086(), new F087(), new F088(), new F089(), new F090(), new F091(), new F092(), new F093(), new F094(), new F095(), new F096(), new F097(), new F098(), new F099(), new F100(), new F101(), new F102(), new F103(), new F104(), new F105(), new F106(), new F107(), new F108(), new F109(), new F110(), new F111(), new F112(), new F113(), new F114(), new F115(), new F116(), new F117(), new F118(), new F119(), new F120(), new F121(), new F122(), new F123(), new F124(), new F125(), new F126(), new F127(), new F128(), new F129(), new F130(), new F131(), new F132(), new F133(), new F134(), new F135(), new F136(), new F137(), new F138(), new F139(), new F140(), new F141(), new F142(), new F143(), new F144(), new F145(), new F146(), new F147(), new F148(), new F149(), new F150(), new F151(), new F152(), new F153(), new F154(), new F155(), new F156(), new F157(), new F158(), new F159(), new F160(), new F161(), new F162(), new F163(), new F164(), new F165(), new F166(), new F167(), new F168(), new F169(), new F170(), new F171(), new F172(), new F173(), new F174(), new F175(), new F176(), new F177(), new F178(), new F179(), new F180(), new F181(), new F182(), new F183(), new F184(), new F185(), new F186(), new F187(), new F188(), new F189(), new F190(), new F191(), new F192(), new F193(), new F194(), new F195(), new F196(), new F197(), new F198(), new F199() };
        // Keys of the table's own type mostly, some of another type or of another class.
        Object[] keys = new Object[4096];
        for (int i = 0; i < keys.length; i++) {
            int kind = i % tables.length;
            int pick = (i * 7 + i / 13) % 16;
            keys[i] = pick == 15 ? others[(i / 16) % others.length]
                : pick == 14 ? Shape.values()[i % 4]
                : kind == 0 ? Color.values()[i % 8]
                : kind == 1 ? Shape.values()[i % 4]
                : kind == 2 ? Size.values()[i % 3]
                : Op.values()[i % 3];
        }
        long check = 0;
        for (int warm = 0; warm < 3; warm++) {
            check += run(tables, keys, 1000);
        }
        long start = System.nanoTime();
        long result = run(tables, keys, 20000);
        long elapsed = (System.nanoTime() - start) / 1000000;
        System.out.println("enum key lookups " + result + " " + check);
        System.err.println("kernel-ms " + elapsed);
    }
}
