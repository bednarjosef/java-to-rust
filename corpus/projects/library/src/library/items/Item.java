package library.items;

public interface Item {
    int LOAN_DAYS = 14;

    String title();

    int days();

    default boolean overdue(int held) {
        return held > days();
    }

    default String describe() {
        return title() + " (" + days() + " days)";
    }

    static int fine(Item item, int held) {
        return item.overdue(held) ? (held - item.days()) * 10 : 0;
    }
}
