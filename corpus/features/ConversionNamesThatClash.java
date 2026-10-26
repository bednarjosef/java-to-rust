// Classes whose names come to one name in snake case, held as `Object` and cast back, so that
// each has a conversion from `Object`: `CSS.Value` beside `CSSValue` and another `Value` (as
// Swing's HTML package has them), and JNA's `WinDef.LONG` beside `Long` and `long[]`, with an
// array of each.
public class ConversionNamesThatClash {
    static String describe(Object o) {
        if (o instanceof CSS.Value v) {
            return "value " + v;
        }
        if (o instanceof CSSValue v) {
            return "css value " + v;
        }
        if (o instanceof Units.Value v) {
            return "units value " + v;
        }
        if (o instanceof WinDef.LONG l) {
            return "win " + l;
        }
        if (o instanceof Long l) {
            return "boxed " + (l + 1);
        }
        if (o instanceof long[] a) {
            return "longs " + a.length;
        }
        if (o instanceof WinDef.LONG[] a) {
            return "wins " + a.length + " " + a[0];
        }
        return "other";
    }

    public static void main(String[] args) {
        Object[] all = {
            new CSS.Value(), new CSSValue(), new Units.Value(), new WinDef.LONG(7), 41L, new long[3],
            new WinDef.LONG[] {new WinDef.LONG(2)}, "text"
        };
        for (Object o : all) {
            System.out.println(describe(o));
        }
    }
}

class CSS {
    static class Value {
        public String toString() {
            return "CSS.Value";
        }
    }
}

class CSSValue {
    public String toString() {
        return "CSSValue";
    }
}

class Units {
    static class Value {
        public String toString() {
            return "Units.Value";
        }
    }
}

class WinDef {
    static class LONG {
        final long value;

        LONG(long value) {
            this.value = value;
        }

        public String toString() {
            return "LONG " + value;
        }
    }
}
