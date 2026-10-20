import java.util.Arrays;
class Item {
    String name; int qty;
    Item(String name, int qty) { this.name = name; this.qty = qty; }
}
class Shelf {
    Item[] slots;
    int used;
    Shelf(int n) { slots = new Item[n]; }
    void put(Item it) { slots[used] = it; used++; }
    Item find(String name) {
        for (int i = 0; i < used; i++) if (slots[i].name.equals(name)) return slots[i];
        return null;
    }
}
public class NullElements {
    static int total(Item[] items) {
        int t = 0;
        for (Item it : items) if (it != null) t += it.qty;
        return t;
    }
    public static void main(String[] args) {
        Shelf s = new Shelf(4);
        s.put(new Item("apple", 3));
        s.put(new Item("pear", 5));
        System.out.println(total(s.slots));
        Item p = s.find("pear");
        System.out.println(p.name + " " + p.qty + " " + (s.find("fig") == null));
        int[][] tri = new int[4][];
        for (int i = 0; i < tri.length; i++) {
            tri[i] = new int[i + 1];
            tri[i][0] = 1; tri[i][i] = 1;
            for (int j = 1; j < i; j++) tri[i][j] = tri[i - 1][j - 1] + tri[i - 1][j];
        }
        System.out.println(Arrays.toString(tri[3]));
        String[] words = new String[3];
        words[1] = "mid";
        System.out.println(Arrays.toString(words));
        words[0] += "!";
        System.out.println(words[0] + " " + args.length);
        int[][] jag = new int[2][];
        System.out.println(jag[1] == null);
        System.out.println(jag[1].length);
    }
}
