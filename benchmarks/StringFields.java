// Many objects whose fields hold strings that other objects hold too, as the Minecraft server's
// identifiers do ("minecraft" and a path that several registries share): a million of them, all
// alive at once, of 400 distinct paths. Java keeps one string object per text and a reference to
// it in each field. What it measures is the memory they take (`scripts/benchmark.py` reports the
// peak), and the time of looking them up by text.
import java.util.HashMap;
import java.util.Map;

public class StringFields {
    static final class Id {
        final String namespace;
        final String path;

        Id(String namespace, String path) {
            this.namespace = namespace;
            this.path = path;
        }

        @Override
        public boolean equals(Object o) {
            return o instanceof Id other && namespace.equals(other.namespace) && path.equals(other.path);
        }

        @Override
        public int hashCode() {
            return 31 * namespace.hashCode() + path.hashCode();
        }
    }

    public static void main(String[] args) {
        String[] paths = new String[400];
        for (int i = 0; i < paths.length; i++) {
            paths[i] = "block/stone_variant_" + i;
        }
        Id[] ids = new Id[1_000_000];
        for (int i = 0; i < ids.length; i++) {
            ids[i] = new Id("minecraft", paths[i % paths.length]);
        }
        Map<Id, Integer> first = new HashMap<>();
        for (int i = 0; i < paths.length; i++) {
            first.put(ids[i], i);
        }
        long sum = 0;
        for (int round = 0; round < 5; round++) {
            for (Id id : ids) {
                sum += first.get(id) + id.path.length();
            }
        }
        System.out.println(sum + " " + first.size());
    }
}
