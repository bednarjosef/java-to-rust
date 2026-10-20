enum Color { RED, GREEN, BLUE }

public class Enums {
    static Color favourite = Color.GREEN;

    static class Paint {
        private final Color color;
        private final int coats;

        Paint(Color color, int coats) {
            this.color = color;
            this.coats = coats;
        }

        Color color() {
            return color;
        }

        String label() {
            return color + " x" + coats;
        }
    }

    static Color next(Color c) {
        return Color.values()[(c.ordinal() + 1) % Color.values().length];
    }

    static String describe(Color c, int times) {
        String text = "";
        for (int i = 0; i < times; i++) {
            text = text + c.name() + ";";
        }
        return text;
    }

    static Color parse(String name) {
        Color found = Color.valueOf(name);
        return found;
    }

    public static void main(String[] args) {
        Color c = Color.RED;
        System.out.println(c);
        System.out.println(c.name() + " " + c.ordinal());
        System.out.println(c == Color.RED);
        System.out.println(c != Color.BLUE);
        System.out.println(next(c));
        System.out.println(next(Color.BLUE));
        System.out.println(favourite);
        System.out.println(Color.valueOf("BLUE").compareTo(Color.RED));
        System.out.println(Color.RED.compareTo(Color.BLUE));
        System.out.println(Color.GREEN.compareTo(Color.GREEN));
        System.out.println(describe(Color.BLUE, 2));
        System.out.println(parse("GREEN") == favourite);
        Paint paint = new Paint(Color.BLUE, 2);
        System.out.println(paint.color() + " " + paint.label());
        System.out.println(Color.values().length);
        for (Color each : Color.values()) {
            System.out.print(each + "=" + each.ordinal() + " ");
        }
        System.out.println();
        System.out.println(Color.valueOf("PINK"));
        System.out.println("not reached");
    }
}
