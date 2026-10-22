public class ConcatOfNullableStrings {
    static class Offset {
        final int total;
        final String id;

        Offset(int total, String id) {
            this.total = total;
            this.id = id;
        }

        int getTotalSeconds() {
            return total;
        }

        String getId() {
            return id;
        }
    }

    static String withOffset(String prefix, Offset offset) {
        try {
            if (offset.getTotalSeconds() != 0) {
                prefix = prefix.concat(offset.getId());
            }
            return prefix;
        } catch (NullPointerException e) {
            return "npe";
        }
    }

    static String resolve(String path, String spec, boolean authority) {
        int ind = path.lastIndexOf('/');
        if (ind == -1 && authority) {
            path = "/".concat(spec);
        } else {
            path = path.substring(0, ind + 1).concat(spec);
        }
        return path;
    }

    public static void main(String[] args) {
        System.out.println(withOffset("GMT", new Offset(3600, "+01:00")));
        System.out.println(withOffset("GMT", new Offset(0, "+01:00")));
        System.out.println(withOffset("GMT", new Offset(60, null)));
        System.out.println(resolve("a/b/c", "d", false));
        System.out.println(resolve("", "d", true));
    }
}
