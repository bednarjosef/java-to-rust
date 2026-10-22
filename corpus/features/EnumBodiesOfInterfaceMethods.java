// An enum whose constants' bodies implement methods of its interface that the enum itself does
// not declare: a default method that some constants override and the others run, and an
// abstract one that every constant implements.
public class EnumBodiesOfInterfaceMethods {
    interface Field {
        String unit();

        default String label(int width) {
            return "field " + unit() + " of width " + width;
        }

        default void show(StringBuilder out) {
            out.append('<').append(unit()).append('>');
        }

        int base();
    }

    enum Clock implements Field {
        HOURS {
            @Override
            public String label(int w) {
                return "hours, " + w + " wide";
            }

            public int base() {
                return 24;
            }
        },
        MINUTES {
            public int base() {
                return 60;
            }

            @Override
            public void show(StringBuilder out) {
                out.append("[min]");
            }
        },
        SECONDS {
            public int base() {
                return 60 * scale();
            }
        };

        int scale() {
            return 1;
        }

        public String unit() {
            return name().toLowerCase();
        }
    }

    public static void main(String[] args) {
        StringBuilder out = new StringBuilder();
        for (Field f : Clock.values()) {
            System.out.println(f.label(f.base()));
            f.show(out);
        }
        System.out.println(out);
        System.out.println(Clock.SECONDS.label(1) + " " + Clock.HOURS.base());
    }
}
