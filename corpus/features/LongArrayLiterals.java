// Long array literals of string constants and of class constants, which the translation makes
// from a table of the constants, and a long literal that mixes them with other values.
public class LongArrayLiterals {
    interface Shape {}
    static class Circle implements Shape {}
    static class Square implements Shape {}

    static final String[] WORDS = {
        "alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta", "theta",
        "iota", "kappa", "lambda", "mu", "nu", "xi", "omicron", "pi",
        "rho", "sigma", "tau", "upsilon", "phi", "chi", "psi", "omega",
        "a \"quoted\" word", "tab\there", "line\nbreak", "back\\slash", "été", "中文", "", "alpha",
        "last"
    };

    static Object[] keys() {
        return new Object[] {
            "k0", "k1", "k2", "k3", "k4", "k5", "k6", "k7", "k8", "k9",
            "k10", "k11", "k12", "k13", "k14", "k15", "k16", "k17", "k18", "k19",
            "k20", "k21", "k22", "k23", "k24", "k25", "k26", "k27", "k28", "k29",
            "k30", "k31", "k32", "k33"
        };
    }

    static Class<?>[] classes() {
        return new Class<?>[] {
            String.class, Integer.class, Long.class, Short.class, Byte.class, Character.class,
            Boolean.class, Double.class, Float.class, Object.class, Number.class, Shape.class,
            Circle.class, LongArrayLiterals.class, Square.class, Circle[].class, Shape.class,
            Square.class, Circle.class, Shape.class, Square.class,
            int.class, long.class, short.class, byte.class, char.class, boolean.class,
            double.class, float.class, void.class, Exception.class, RuntimeException.class,
            Error.class, Throwable.class
        };
    }

    static Object[] mixed(int n) {
        return new Object[] {
            "m0", "m1", "m2", "m3", "m4", "m5", "m6", "m7", "m8", "m9",
            "m10", "m11", "m12", "m13", "m14", "m15", "m16", "m17", "m18", "m19",
            "m20", "m21", "m22", "m23", "m24", "m25", "m26", "m27", "m28", "m29",
            String.class, n, null, "m33"
        };
    }

    public static void main(String[] args) {
        System.out.println(WORDS.length + " " + String.join(",", WORDS));
        System.out.println(WORDS[0] == WORDS[31]);
        WORDS[1] = "changed";
        System.out.println(WORDS[1] + " " + keys()[1] + " " + (keys() != keys()));
        Object[] k = keys();
        k[0] = 42;
        System.out.println(k.length + " " + k[0] + " " + k[33] + " " + k.getClass().getSimpleName());
        StringBuilder names = new StringBuilder();
        for (Class<?> c : classes()) {
            names.append(c.getSimpleName()).append(' ');
        }
        System.out.println(classes().length + " " + names.toString().trim());
        System.out.println(classes()[13] == LongArrayLiterals.class);
        StringBuilder all = new StringBuilder();
        for (Object o : mixed(args.length)) {
            all.append(o).append(' ');
        }
        System.out.println(all.toString().trim());
    }
}
