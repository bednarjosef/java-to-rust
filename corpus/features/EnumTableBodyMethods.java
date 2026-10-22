// An enum whose constructor runs code (its fields are tables) and whose methods some constants'
// bodies override, called through the enum type; a body's helper, which only its own override
// calls, reads the enum's table of its constant.
public enum EnumTableBodyMethods {
    PLAIN("plain", 1),
    WIDE("wide", 2) {
        @Override
        String label() {
            return "[" + helper() + "] " + name + "/" + width;
        }

        private String helper() {
            return "w" + width + aliases.length;
        }
    },
    NARROW("narrow", 3) {
        @Override
        String label() {
            return "<" + name + ">";
        }
    },
    BARE("bare", 4);

    final String name;
    final int width;
    final String[] aliases;

    EnumTableBodyMethods(String name, int width) {
        this.name = name;
        this.width = width;
        this.aliases = new String[width];
        for (int i = 0; i < width; i++) {
            aliases[i] = name + i;
        }
    }

    String label() {
        return name + "/" + width;
    }

    String first() {
        return aliases[0];
    }

    public static void main(String[] args) {
        for (EnumTableBodyMethods e : values()) {
            System.out.println(e.label() + " " + e.aliases.length + " " + e.first());
        }
        EnumTableBodyMethods w = WIDE;
        System.out.println(w.label() + " " + w.first());
    }
}
