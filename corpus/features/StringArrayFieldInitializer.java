public class StringArrayFieldInitializer {
    static class Bag {
        String[] items = new String[2];
        int count;

        void add(String item) {
            items[count] = item;
            count++;
        }
    }

    static class Grid {
        static String[] names = new String[3];
        String[][] rows = new String[2][2];
        int[] counts = new int[2];

        void fill() {
            rows[1][0] = names[2] == null ? "none" : names[2];
            names[2] = "last";
        }
    }

    public static void main(String[] args) {
        Bag b = new Bag();
        b.add("pen");
        System.out.println(b.items[0] + " " + b.items[1] + " " + b.count);
        Grid g = new Grid();
        System.out.println(g.rows[0][1] + " " + Grid.names[2]);
        g.fill();
        System.out.println(g.rows[1][0] + " " + g.rows[0][1] + " " + Grid.names[2] + " " + g.counts[1]);
    }
}
