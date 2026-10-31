// Keys made of strings, hashed as the Minecraft server hashes its Identifiers (a namespace and a
// path) and the TagKeys made of two of them: a block's tags are a hash set of TagKeys probed on
// every `state.is(tag)`. Java's String keeps its hash, so each probe hashes no text.
public class StringKeyHash {
    static final class Id {
        final String namespace;
        final String path;

        Id(String namespace, String path) {
            this.namespace = namespace;
            this.path = path;
        }

        @Override
        public boolean equals(Object o) {
            return o instanceof Id id && namespace.equals(id.namespace) && path.equals(id.path);
        }

        @Override
        public int hashCode() {
            return 31 * namespace.hashCode() + path.hashCode();
        }
    }

    static final class Tag {
        final Id registry;
        final Id location;

        Tag(Id registry, Id location) {
            this.registry = registry;
            this.location = location;
        }

        @Override
        public boolean equals(Object o) {
            return o instanceof Tag t && registry.equals(t.registry) && location.equals(t.location);
        }

        @Override
        public int hashCode() {
            return 31 * registry.hashCode() + location.hashCode();
        }
    }

    static final class TagSet {
        final Tag[] table;

        TagSet(Tag[] tags) {
            table = new Tag[tags.length * 4];
            for (Tag t : tags) {
                int i = Math.floorMod(t.hashCode() * 0x9E3779B9, table.length);
                while (table[i] != null) {
                    i = (i + 1) % table.length;
                }
                table[i] = t;
            }
        }

        boolean contains(Tag t) {
            int i = Math.floorMod(t.hashCode() * 0x9E3779B9, table.length);
            while (table[i] != null) {
                if (table[i].equals(t)) {
                    return true;
                }
                i = (i + 1) % table.length;
            }
            return false;
        }
    }

    static int probe(TagSet set, Tag[] asked, int rounds) {
        int found = 0;
        for (int r = 0; r < rounds; r++) {
            for (Tag t : asked) {
                if (set.contains(t)) {
                    found++;
                }
            }
        }
        return found;
    }

    public static void main(String[] args) {
        String[] kinds = {"mineable/pickaxe", "mineable/axe", "logs_that_burn", "replaceable_by_trees", "climbable",
                "fall_damage_resetting", "dragon_immune", "wither_immune", "snow_layer_can_survive_on"};
        Id blocks = new Id("minecraft", "block");
        Tag[] tags = new Tag[200];
        Tag[] asked = new Tag[400];
        for (int i = 0; i < tags.length; i++) {
            tags[i] = new Tag(blocks, new Id("minecraft", kinds[i % kinds.length] + "_" + i));
        }
        for (int i = 0; i < asked.length; i++) {
            // Equal keys made apart from the set's, as a lookup's key is.
            asked[i] = new Tag(new Id("minecraft", "block"), new Id("minecraft", kinds[i % kinds.length] + "_" + (i * 7 % 300)));
        }
        TagSet set = new TagSet(tags);
        int warm = probe(set, asked, 200);
        long start = System.nanoTime();
        int found = probe(set, asked, 5000);
        System.err.println("kernel-ms " + (System.nanoTime() - start) / 1000000);
        System.out.println(warm + " " + found);
    }
}
