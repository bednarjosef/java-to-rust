// Fields named `outer` in inner classes, as javac's ModuleFinder.ModuleLocationIterator has: the
// translation's own field for the enclosing instance takes another name.
public class FieldsNamedOuter {
    private final String label;

    FieldsNamedOuter(String label) {
        this.label = label;
    }

    class Locations {
        String outer;
        final String[] outers = {"source", "class", "module"};
        int at;

        boolean hasNext() {
            return at < outers.length;
        }

        String next() {
            outer = outers[at++];
            return label + ":" + outer;
        }
    }

    class Shadowing {
        int outer = 7;

        String show(int outer) {
            return label + " " + outer + " " + this.outer;
        }
    }

    class Derived extends Shadowing {
        String describe() {
            return label + " derived " + outer;
        }
    }

    void runAnonymous() {
        Runnable r = new Runnable() {
            final String outer = "anonymous";

            @Override
            public void run() {
                System.out.println(label + " " + outer);
            }
        };
        r.run();
    }

    public static void main(String[] args) {
        FieldsNamedOuter f = new FieldsNamedOuter("paths");
        Locations it = f.new Locations();
        while (it.hasNext()) {
            System.out.println(it.next());
        }
        System.out.println(it.outer);
        System.out.println(f.new Shadowing().show(3));
        System.out.println(f.new Derived().describe());
        f.runAnonymous();
    }
}
