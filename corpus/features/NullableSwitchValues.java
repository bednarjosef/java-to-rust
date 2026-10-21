// A switch expression one of whose values may be null is null then, as ImmutableCollections's
// `listFromTrustedArray` returns a static that its initializer may leave unset.
public class NullableSwitchValues {
    static class Items {
        final String text;

        Items(String text) {
            this.text = text;
        }

        public String toString() {
            return "[" + text + "]";
        }
    }

    static Items empty;
    static String fallback = null;

    static Items itemsOf(String... items) {
        return switch (items.length) {
            case 0 -> empty;
            case 1 -> new Items(items[0]);
            default -> {
                String all = "";
                for (String s : items) {
                    all += s;
                }
                yield new Items(all);
            }
        };
    }

    static String name(int code) {
        String found = switch (code) {
            case 1 -> "one";
            case 2 -> {
                if (fallback == null) {
                    yield null;
                }
                yield fallback;
            }
            default -> "many";
        };
        return found;
    }

    public static void main(String[] args) {
        System.out.println(itemsOf() == null);
        empty = new Items("");
        System.out.println(itemsOf() + " " + itemsOf("a") + " " + itemsOf("b", "c"));
        System.out.println(name(1) + " " + name(2) + " " + name(3));
        fallback = "two";
        System.out.println(name(2).length());
    }
}
