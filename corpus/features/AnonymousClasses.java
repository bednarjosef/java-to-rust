public class AnonymousClasses {
    interface Order {
        int compare(String a, String b);
    }

    interface Source {
        boolean more();
        int take();
    }

    interface Greeting {
        String text();
    }

    static void sort(String[] items, Order order) {
        for (int i = 1; i < items.length; i++) {
            String item = items[i];
            int j = i - 1;
            while (j >= 0 && order.compare(items[j], item) > 0) {
                items[j + 1] = items[j];
                j--;
            }
            items[j + 1] = item;
        }
    }

    static String join(String[] items) {
        StringBuilder sb = new StringBuilder();
        for (String s : items) sb.append(s).append(' ');
        return sb.toString().trim();
    }

    // An anonymous source with fields of its own, counting down from a captured parameter.
    static Source countdown(int from, int step) {
        return new Source() {
            int left = from;
            int taken;
            public boolean more() { return left > 0; }
            public int take() {
                taken++;
                int v = left;
                left -= step;
                return v * 10 + taken;
            }
        };
    }

    String name = "outer";
    int greeted;

    // An anonymous class that captures a local and reads and writes the enclosing object.
    Greeting greeter(String punctuation) {
        return new Greeting() {
            public String text() {
                greeted++;
                return "hello from " + name + punctuation + " #" + greeted;
            }
        };
    }

    // An inner class whose method creates an anonymous class reaching the outermost object.
    class Helper {
        String suffix = "!";
        Greeting make() {
            return new Greeting() {
                public String text() { return AnonymousClasses.this.name + suffix + " via helper"; }
            };
        }
    }

    public static void main(String[] args) {
        String[] words = { "pear", "fig", "banana", "kiwi", "apple" };
        // Two anonymous classes of one interface in one method.
        sort(words, new Order() {
            public int compare(String a, String b) { return a.compareTo(b); }
        });
        System.out.println(join(words));
        int[] calls = new int[1];
        sort(words, new Order() {
            public int compare(String a, String b) {
                calls[0]++;
                return a.length() != b.length() ? a.length() - b.length() : b.compareTo(a);
            }
        });
        System.out.println(join(words) + " after " + calls[0] + " comparisons");

        Source s = countdown(7, 3);
        StringBuilder taken = new StringBuilder();
        while (s.more()) taken.append(s.take()).append(',');
        System.out.println(taken);

        AnonymousClasses o = new AnonymousClasses();
        Greeting g = o.greeter("?");
        System.out.println(g.text());
        o.name = "renamed";
        System.out.println(g.text());
        Greeting h = o.new Helper().make();
        System.out.println(h.text() + " " + o.greeted);
    }
}
