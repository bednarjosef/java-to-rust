// Enum constants with bodies that override the enum's methods: an abstract method that every
// constant implements, a method only some constants override (the others run the enum's own),
// a void method, and overrides that name their parameters differently, read the enum's fields
// and call its other methods.
public class EnumConstantBodies {
    enum Operation {
        PLUS("+") {
            int apply(int a, int b) {
                return a + b;
            }
        },
        MINUS("-") {
            int apply(int left, int right) {
                return left - right;
            }

            boolean commutative() {
                return false;
            }
        },
        TIMES("*") {
            int apply(int a, int b) {
                int product = 0;
                for (int i = 0; i < b; i++) {
                    product += a;
                }
                return product;
            }
        },
        POWER("^") {
            int apply(int base, int exponent) {
                return exponent == 0 ? 1 : base * apply(base, exponent - 1);
            }

            boolean commutative() {
                return false;
            }

            void explain(int a, int b) {
                System.out.println("  " + a + " multiplied by itself " + b + " times");
            }
        };

        private final String symbol;

        Operation(String symbol) {
            this.symbol = symbol;
        }

        abstract int apply(int a, int b);

        boolean commutative() {
            return true;
        }

        void explain(int a, int b) {
            System.out.println("  " + a + " " + symbol + " " + b + " = " + apply(a, b));
        }

        String show(int a, int b) {
            return a + " " + symbol + " " + b + " = " + apply(a, b) + (commutative() ? " (commutative)" : "");
        }
    }

    enum Light {
        RED {
            Light next() {
                return GREEN;
            }
        },
        GREEN {
            Light next() {
                return YELLOW;
            }
        },
        YELLOW {
            Light next() {
                return RED;
            }
        };

        abstract Light next();
    }

    public static void main(String[] args) {
        for (Operation op : Operation.values()) {
            System.out.println(op + ": " + op.show(3, 4));
            op.explain(2, 5);
        }
        Light light = Light.RED;
        for (int i = 0; i < 5; i++) {
            System.out.print(light + " ");
            light = light.next();
        }
        System.out.println(Light.valueOf("GREEN").next().ordinal());
        System.out.println(Operation.valueOf("MINUS").apply(10, 3));
    }
}
