// Records, enums and interfaces declared in initializer blocks, static and instance, move to the
// top level as those declared in methods do.
public class LocalTypesInBlocks {
    static final String STATIC_TEXT;
    final String text;

    static {
        enum Shade {
            LIGHT,
            DARK
        }
        record Pair(Shade shade, int depth) {}
        STATIC_TEXT = new Pair(Shade.DARK, 2) + " " + Shade.values().length;
    }

    {
        interface Greeter {
            String greet(String name);
        }
        record Point(int x, int y) {}
        Greeter g = name -> "hi " + name;
        text = g.greet("block") + " " + new Point(3, 4);
    }

    public static void main(String[] args) {
        System.out.println(STATIC_TEXT);
        System.out.println(new LocalTypesInBlocks().text);
    }
}
