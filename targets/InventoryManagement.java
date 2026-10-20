// Features: an inventory domain model with a checked custom exception and an unchecked one,
// HashMap<String, Item> keyed by SKU, TreeMap for sorted reports, ArrayList of transactions, a
// small Optional-returning lookup, String.format with width and precision, BigDecimal-free money in
// cents, enhanced for loops, switch on an enum of transaction kinds, and a summary report.
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.TreeMap;

public class InventoryManagement {
    static class OutOfStockException extends Exception {
        private final String sku;
        private final int missing;

        OutOfStockException(String sku, int missing) {
            super("sku " + sku + " is short by " + missing);
            this.sku = sku;
            this.missing = missing;
        }

        String sku() {
            return sku;
        }

        int missing() {
            return missing;
        }
    }

    static class UnknownSkuException extends RuntimeException {
        UnknownSkuException(String sku) {
            super("unknown sku: " + sku);
        }
    }

    enum Kind {
        RECEIVE, SHIP, ADJUST
    }

    static class Item {
        final String sku;
        final String title;
        final int priceCents;
        int quantity;

        Item(String sku, String title, int priceCents, int quantity) {
            this.sku = sku;
            this.title = title;
            this.priceCents = priceCents;
            this.quantity = quantity;
        }

        int valueCents() {
            return priceCents * quantity;
        }
    }

    record Transaction(Kind kind, String sku, int amount) {
    }

    static class Warehouse {
        private final Map<String, Item> items = new HashMap<>();
        private final List<Transaction> log = new ArrayList<>();

        void add(Item item) {
            items.put(item.sku, item);
        }

        Optional<Item> find(String sku) {
            return Optional.ofNullable(items.get(sku));
        }

        Item require(String sku) {
            Item item = items.get(sku);
            if (item == null) {
                throw new UnknownSkuException(sku);
            }
            return item;
        }

        void apply(Transaction t) throws OutOfStockException {
            Item item = require(t.sku());
            switch (t.kind()) {
                case RECEIVE -> item.quantity += t.amount();
                case SHIP -> {
                    if (item.quantity < t.amount()) {
                        throw new OutOfStockException(t.sku(), t.amount() - item.quantity);
                    }
                    item.quantity -= t.amount();
                }
                case ADJUST -> item.quantity = t.amount();
            }
            log.add(t);
        }

        int totalValueCents() {
            int total = 0;
            for (Item item : items.values()) {
                total += item.valueCents();
            }
            return total;
        }

        Map<String, Integer> quantitiesSorted() {
            Map<String, Integer> sorted = new TreeMap<>();
            for (Item item : items.values()) {
                sorted.put(item.title, item.quantity);
            }
            return sorted;
        }

        int logSize() {
            return log.size();
        }
    }

    static String money(int cents) {
        return String.format("$%d.%02d", cents / 100, cents % 100);
    }

    public static void main(String[] args) {
        Warehouse w = new Warehouse();
        w.add(new Item("A-100", "Widget", 250, 40));
        w.add(new Item("B-200", "Gadget", 1299, 3));
        w.add(new Item("C-300", "Doohickey", 75, 0));

        List<Transaction> plan = List.of(
                new Transaction(Kind.SHIP, "A-100", 15),
                new Transaction(Kind.SHIP, "B-200", 5),
                new Transaction(Kind.RECEIVE, "C-300", 120),
                new Transaction(Kind.ADJUST, "B-200", 8),
                new Transaction(Kind.SHIP, "C-300", 20),
                new Transaction(Kind.SHIP, "Z-999", 1));

        int failures = 0;
        for (Transaction t : plan) {
            try {
                w.apply(t);
                System.out.println("ok: " + t);
            } catch (OutOfStockException e) {
                failures++;
                System.out.println("stock problem for " + e.sku() + " (" + e.missing() + " missing): " + e.getMessage());
            } catch (UnknownSkuException e) {
                failures++;
                System.out.println("lookup failed: " + e.getMessage());
            } finally {
                System.out.println("  log size now " + w.logSize());
            }
        }
        System.out.println("failures: " + failures);

        System.out.println(String.format("%-12s|%8s|%5s", "title", "price", "qty"));
        for (String title : w.quantitiesSorted().keySet()) {
            System.out.println(String.format("%-12s|%8s|%5d", title, "", w.quantitiesSorted().get(title)));
        }
        for (String sku : List.of("A-100", "B-200", "C-300")) {
            Item item = w.require(sku);
            System.out.printf("%s %-10s %10s x %3d = %10s%n", item.sku, item.title, money(item.priceCents),
                    item.quantity, money(item.valueCents()));
        }
        System.out.println("total value: " + money(w.totalValueCents()));
        System.out.println("find A-100 present: " + w.find("A-100").isPresent());
        System.out.println("find Q-1 title: " + w.find("Q-1").map(i -> i.title).orElse("<none>"));
    }
}
