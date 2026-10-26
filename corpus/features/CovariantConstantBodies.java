// Enum constants' bodies whose methods narrow the return type of the method they override: of an
// interface the enum leaves to its constants (Gson's `ToNumberPolicy`), of an abstract method of
// the enum (Guava's `Types.JavaVersion`), and of a method of the enum that some constants keep.
public class CovariantConstantBodies {
    interface Reader {
        Number read(String text);
    }

    enum Policy implements Reader {
        WHOLE {
            @Override
            public Integer read(String text) {
                return Integer.parseInt(text);
            }
        },
        LONG {
            @Override
            public Number read(String text) {
                return Long.parseLong(text) * 3;
            }
        },
        HALF {
            @Override
            public Number read(String text) {
                return Double.parseDouble(text) / 2;
            }
        },
        THOUSANDS {
            @Override
            public Double read(String text) {
                return Double.parseDouble(text) * 1000;
            }
        };
    }

    enum Version {
        OLD {
            @Override
            CharSequence make(String s) {
                return new StringBuilder(s).reverse();
            }
        },
        NEW {
            @Override
            Object make(String s) {
                return s.toUpperCase();
            }
        },
        NEWER {
            @Override
            String make(String s) {
                return s + s;
            }
        };

        abstract Object make(String s);
    }

    enum Shape {
        SQUARE {
            @Override
            String describe() {
                return "four sides";
            }
        },
        CIRCLE;

        CharSequence describe() {
            return "round";
        }
    }

    public static void main(String[] args) {
        for (Policy p : Policy.values()) {
            Reader r = p;
            System.out.println(p + " " + r.read("12") + " " + p.read("8"));
        }
        System.out.println(Policy.WHOLE.read("7").intValue() + 1);
        for (Version v : Version.values()) {
            System.out.println(v + " " + v.make("ab"));
        }
        for (Shape s : Shape.values()) {
            System.out.println(s + " " + s.describe());
        }
    }
}
