/// The `a == b || .. a.equals(b) ..` shortcut with a field of a local as an operand, `e.key`, and
/// its negated form. A user class counts its equals calls, so the output shows that equals runs
/// exactly when the identities differ.
public class IdentityShortcutFields {
    static int calls;

    static class Token {
        final String name;

        Token(String name) {
            this.name = name;
        }

        @Override
        public boolean equals(Object o) {
            calls++;
            return o instanceof Token t && t.name.equals(name);
        }

        @Override
        public int hashCode() {
            return name.hashCode();
        }

        @Override
        public String toString() {
            return "Token(" + name + ")";
        }
    }

    static class Entry {
        Object key;
        Object value;
        Entry next;

        Entry(Object key, Object value, Entry next) {
            this.key = key;
            this.value = value;
            this.next = next;
        }
    }

    static boolean hasKey(Entry e, Object key) {
        return e.key == key || (key != null && key.equals(e.key));
    }

    static boolean hasKeySwapped(Entry e, Object key) {
        return key == e.key || (e.key != null && e.key.equals(key));
    }

    static boolean keyDiffers(Entry e, Object key) {
        return e.key != key && (key == null || !key.equals(e.key));
    }

    static boolean keyDiffersSwapped(Entry e, Object key) {
        return key != e.key && (e.key == null || !e.key.equals(key));
    }

    // The field read in a loop, as HashMap's bins walk their entries.
    static int count(Entry first, Object key) {
        int n = 0;
        for (Entry e = first; e != null; e = e.next) {
            if (e.key == key || (key != null && key.equals(e.key))) {
                n++;
            }
        }
        return n;
    }

    // The field of a local on both sides: both reads are of the same value.
    static boolean sameKey(Entry a, Entry b) {
        return a.key == b.key || (a.key != null && a.key.equals(b.key));
    }

    public static void main(String[] args) {
        String word = "word";
        Token t = new Token("t");
        Entry chain = new Entry(new String("word"), 1, new Entry(null, 2, new Entry(t, 3, new Entry(7, 4, null))));

        System.out.println("count word " + count(chain, word) + " calls " + calls);
        System.out.println("count null " + count(chain, null) + " calls " + calls);
        System.out.println("count t " + count(chain, t) + " calls " + calls);
        System.out.println("count token " + count(chain, new Token("t")) + " calls " + calls);
        System.out.println("count boxed " + count(chain, Integer.valueOf(7)) + " calls " + calls);

        Entry first = chain;
        Entry second = chain.next;
        Entry third = second.next;
        System.out.println(hasKey(first, "word") + " " + hasKey(second, null) + " " + hasKey(second, word));
        System.out.println(hasKey(third, t) + " " + hasKey(third, new Token("u")) + " calls " + calls);
        System.out.println(hasKeySwapped(first, "word") + " " + hasKeySwapped(third, t) + " calls " + calls);
        System.out.println(hasKeySwapped(second, "x") + " " + hasKeySwapped(second, null) + " calls " + calls);
        System.out.println(keyDiffers(first, word) + " " + keyDiffers(second, null) + " " + keyDiffers(third, t));
        System.out.println(keyDiffers(second, word) + " calls " + calls);
        System.out.println(keyDiffersSwapped(first, word) + " " + keyDiffersSwapped(second, null));
        System.out.println(keyDiffersSwapped(third, t) + " " + keyDiffersSwapped(third, "t") + " calls " + calls);

        System.out.println(sameKey(first, chain.next.next.next) + " " + sameKey(second, third));
        System.out.println(sameKey(third, new Entry(t, 0, null)) + " calls " + calls);
        System.out.println(sameKey(new Entry(null, 0, null), second) + " " + sameKey(first, second));
    }
}
