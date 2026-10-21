public class InstanceOf {
    interface Item {
        String label();
    }

    enum Suit implements Item {
        HEARTS, SPADES;
        public String label() { return name().toLowerCase(); }
    }

    static class Card implements Item {
        int rank;
        Card(int rank) { this.rank = rank; }
        public String label() { return "card " + rank; }
        boolean isJoker() { return this instanceof Joker; }
    }

    static class Joker extends Card {
        Joker() { super(0); }
        public String label() { return "joker"; }
    }

    static class Link {
        Item item;
        Link next;
        Link(Item item, Link next) { this.item = item; this.next = next; }
    }

    static class Holder {
        Item held;
        Holder(Item held) { this.held = held; }

        int rank() {
            if (held instanceof Card c) {
                return c.rank;
            }
            return -1;
        }

        void bump() {
            if (held instanceof Card c && !(held instanceof Joker)) {
                c.rank++;
            }
        }
    }

    static int sumRanks(Link list) {
        int sum = 0;
        Link l = list;
        while (l != null) {
            Item it = l.item;
            sum += it instanceof Card c ? c.rank : 0;
            l = l.next;
        }
        return sum;
    }

    static String kind(Item it) {
        if (!(it instanceof Card c)) {
            Suit s = (Suit) it;
            return "suit " + s.ordinal();
        } else {
            return "rank " + c.rank;
        }
    }

    static Item pick(int i) {
        if (i == 0) return Suit.SPADES;
        if (i == 1) return new Joker();
        if (i == 2) return null;
        return new Card(i);
    }

    public static void main(String[] args) {
        Link list = new Link(new Card(3), new Link(Suit.HEARTS, new Link(new Joker(), new Link(new Card(9), null))));
        System.out.println(sumRanks(list));
        for (int i = 0; i < 5; i++) {
            Item it = pick(i);
            System.out.println(i + ": " + (it instanceof Card) + " " + (it instanceof Joker) + " " + (it instanceof Suit));
            if (it != null) System.out.println(it.label());
            Card c = (Card) (i == 0 ? null : it);
            System.out.println(c == null);
        }
        Holder h = new Holder(new Card(4));
        h.bump();
        h.bump();
        System.out.println(h.rank());
        Holder j = new Holder(new Joker());
        j.bump();
        System.out.println(j.rank() + " " + new Holder(Suit.SPADES).rank());
        Card plain = new Card(1);
        Item asItem = plain;
        System.out.println(asItem instanceof Card k && k.rank == 1);
        Joker joker = (Joker) pick(1);
        System.out.println(joker.label());
        Item[] items = { Suit.SPADES, new Joker(), new Card(5) };
        for (Item item : items) System.out.println(kind(item));
        Suit s = (Suit) items[0];
        System.out.println(s);
        // A test against a class that the value's class is not below, and a test of `this`.
        System.out.println(plain instanceof Joker);
        System.out.println(new Joker().isJoker() + " " + plain.isJoker());
        // A cast that fails throws ClassCastException, which ends the program.
        Joker wrong = (Joker) items[2];
        System.out.println("not reached " + wrong.label());
    }
}
