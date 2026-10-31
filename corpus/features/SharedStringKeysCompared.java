// StringKeysCompared in a program whose strings are objects with an identity (the `==` below):
// texts of every length up to 40 that differ in one place, non-ASCII and empty texts, and strings
// with equal hashes ("Aa" and "BB") whose texts differ, before and after their hashes are known.
public class SharedStringKeysCompared {
    record Id(String namespace, String path) {}

    // A set of keys by their `hashCode` and `equals`, as a hash set keeps them.
    static final class Keys {
        final Object[] table = new Object[1024];
        int size;

        boolean add(Object key) {
            int i = Math.floorMod(key.hashCode(), table.length);
            while (table[i] != null) {
                if (table[i].equals(key)) {
                    return false;
                }
                i = (i + 1) % table.length;
            }
            table[i] = key;
            size++;
            return true;
        }

        boolean contains(Object key) {
            int i = Math.floorMod(key.hashCode(), table.length);
            while (table[i] != null) {
                if (table[i].equals(key)) {
                    return true;
                }
                i = (i + 1) % table.length;
            }
            return false;
        }
    }

    static String text(int n, int from) {
        StringBuilder b = new StringBuilder();
        for (int i = 0; i < n; i++) {
            b.append((char) ('a' + (from + i) % 26));
        }
        return b.toString();
    }

    static String changed(String s, int at, char c) {
        return s.substring(0, at) + c + s.substring(at + 1);
    }

    public static void main(String[] args) {
        int equal = 0;
        int differ = 0;
        for (int n = 0; n <= 40; n++) {
            String a = text(n, n);
            String b = text(n, n);
            if (a.equals(b)) {
                equal++;
            }
            for (int i = 0; i < n; i++) {
                String c = changed(a, i, 'Z');
                if (!a.equals(c) && !c.equals(a)) {
                    differ++;
                }
                // Hashes known on one side, then on both.
                a.hashCode();
                if (!a.equals(c)) {
                    differ++;
                }
                c.hashCode();
                if (!c.equals(a)) {
                    differ++;
                }
            }
            if (n > 0 && a.equals(a.substring(1)) || a.equals(a + "x")) {
                System.out.println("a prefix equal at " + n);
            }
        }
        System.out.println("equal " + equal + " differ " + differ);

        String aa = "Aa";
        String bb = "BB";
        System.out.println(aa.hashCode() == bb.hashCode());
        System.out.println(aa.equals(bb) + " " + aa.equals(new String("Aa")));
        // Either the literal or a new string: only the objects decide `==`.
        String made = args.length == 0 ? new String(aa) : aa;
        String same = args.length == 0 ? aa : new String(aa);
        System.out.println((made == aa) + " " + (same == aa) + " " + made.equals(aa) + " " + same.equals(aa));
        String e = "";
        System.out.println(e.equals("") + " " + e.equals(text(0, 3)) + " " + e.hashCode());
        String grin = "čeština 😀 und Grüße";
        System.out.println(grin.equals("čeština 😀 und Grüße") + " " + grin.equals("čeština 😀 und Grüsse"));
        // The high surrogate put back joins the low one after it; an `x` after it leaves it alone.
        System.out.println(grin.hashCode() + " " + grin.equals(changed(grin, 8, '\uD83D')));
        String lone = changed(grin, 9, 'x');
        System.out.println(lone.equals(changed(grin, 9, 'x')) + " " + lone.equals(grin) + " " + lone.hashCode());

        Keys seen = new Keys();
        for (int i = 0; i < 300; i++) {
            seen.add("key_" + (i % 97) + "_" + text(i % 7, i));
        }
        System.out.println(seen.size + " " + seen.contains("key_3_def") + " " + seen.contains("key_3_deg"));

        Keys ids = new Keys();
        String[] kinds = {"mineable/pickaxe", "logs_that_burn", "climbable", "", "größe"};
        int added = 0;
        for (int i = 0; i < 50; i++) {
            if (ids.add(new Id("minecraft", kinds[i % kinds.length] + (i % 10)))) {
                added++;
            }
        }
        int found = 0;
        for (int i = 0; i < 60; i++) {
            if (ids.contains(new Id(i < 55 ? "minecraft" : "minecraftx", kinds[i % kinds.length] + (i % 10)))) {
                found++;
            }
        }
        System.out.println(added + " " + ids.size + " " + found + " " + ids.contains(new Id("minecraft", "größe4")));
    }
}
