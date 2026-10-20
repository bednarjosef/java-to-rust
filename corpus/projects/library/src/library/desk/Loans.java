package library.desk;

import library.items.Item;

public class Loans {
    private final Item[] items = new Item[10];
    private final int[] held = new int[10];
    private int count;

    public void lend(Item item, int days) {
        items[count] = item;
        held[count] = days;
        count++;
    }

    public int fines() {
        int total = 0;
        for (int i = 0; i < count; i++) {
            total += Item.fine(items[i], held[i]);
        }
        return total;
    }

    public String report() {
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < count; i++) {
            sb.append(items[i].describe());
            if (items[i].overdue(held[i])) {
                sb.append(" OVERDUE");
            }
            sb.append('\n');
        }
        return sb.toString();
    }

    public boolean has(Item item) {
        for (int i = 0; i < count; i++) {
            if (items[i] == item) {
                return true;
            }
        }
        return false;
    }
}
