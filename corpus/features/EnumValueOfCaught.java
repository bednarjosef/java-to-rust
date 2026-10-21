// An enum with fields, constant bodies and an interface; `valueOf` whose IllegalArgumentException
// is caught, and one that ends the program.
public class EnumValueOfCaught {
    interface Priced {
        int price();
    }

    enum Size implements Priced {
        SMALL(1) {
            @Override
            String label() {
                return "s";
            }
        },
        MEDIUM(5),
        LARGE(9) {
            @Override
            public int price() {
                return base * 2;
            }
        };

        final int base;

        Size(int base) {
            this.base = base;
        }

        String label() {
            return name().toLowerCase();
        }

        public int price() {
            return base;
        }

        static Size biggest() {
            Size best = SMALL;
            for (Size s : values()) {
                if (s.compareTo(best) > 0) best = s;
            }
            return best;
        }
    }

    enum Op {
        ADD {
            int apply(int a, int b) { return a + b; }
        },
        MUL {
            int apply(int a, int b) { return a * b; }
        };

        abstract int apply(int a, int b);
    }

    static Size parse(String text) {
        try {
            return Size.valueOf(text);
        } catch (IllegalArgumentException e) {
            System.out.println("caught " + e.getMessage());
            return Size.MEDIUM;
        }
    }

    public static void main(String[] args) {
        for (Size s : Size.values()) {
            Priced p = s;
            System.out.println(s + " " + s.ordinal() + " " + s.label() + " " + p.price() + " " + s.name());
        }
        System.out.println(Size.biggest() + " " + Size.valueOf("LARGE").price());
        System.out.println(parse("HUGE") + " " + parse("SMALL"));
        System.out.println(Size.SMALL.compareTo(Size.LARGE) + " " + Size.LARGE.compareTo(Size.SMALL));
        for (Op op : Op.values()) {
            System.out.println(op + " " + op.apply(6, 7));
        }
        switch (Size.valueOf("MEDIUM")) {
            case SMALL -> System.out.println("small");
            case MEDIUM -> System.out.println("medium");
            case LARGE -> System.out.println("large");
        }
        Size.valueOf("TINY");
    }
}
