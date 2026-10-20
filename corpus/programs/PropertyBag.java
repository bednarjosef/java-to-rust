// A settings store whose values are `Object`s: strings, numbers, flags and small value classes,
// read back through typed getters that test what each value is.
public class PropertyBag {
    static final class Size {
        final int width;
        final int height;

        Size(int width, int height) {
            this.width = width;
            this.height = height;
        }

        @Override
        public boolean equals(Object o) {
            if (!(o instanceof Size)) {
                return false;
            }
            Size s = (Size) o;
            return s.width == width && s.height == height;
        }

        @Override
        public int hashCode() {
            return width * 1000 + height;
        }

        @Override
        public String toString() {
            return width + "x" + height;
        }
    }

    // A marker without `toString`, which prints as its class name and hash code.
    static final class Unset {
        @Override
        public int hashCode() {
            return 255;
        }
    }

    private final String[] keys = new String[16];
    private final Object[] values = new Object[16];
    private int count;

    void put(String key, Object value) {
        for (int i = 0; i < count; i++) {
            if (keys[i].equals(key)) {
                values[i] = value;
                return;
            }
        }
        keys[count] = key;
        values[count] = value;
        count++;
    }

    Object get(String key) {
        for (int i = 0; i < count; i++) {
            if (keys[i].equals(key)) {
                return values[i];
            }
        }
        return null;
    }

    int getInt(String key, int fallback) {
        Object v = get(key);
        if (v instanceof Integer i) {
            return i;
        }
        if (v instanceof String s && !s.isEmpty() && Character.isDigit(s.charAt(0))) {
            return Integer.parseInt(s);
        }
        return fallback;
    }

    boolean getFlag(String key) {
        return get(key) instanceof Boolean b && b;
    }

    String describe(String key) {
        Object v = get(key);
        if (v == null) {
            return key + " is missing";
        } else if (v instanceof String s) {
            return key + " is the text \"" + s + "\"";
        } else if (v instanceof Integer i) {
            return key + " is the number " + i;
        } else if (v instanceof Double d) {
            return key + " is the decimal " + d;
        } else if (v instanceof Boolean b) {
            return key + " is " + (b ? "on" : "off");
        } else if (v instanceof Size s) {
            return key + " is " + s.width + " wide";
        }
        return key + " is " + v;
    }

    int countEqual(Object value) {
        int n = 0;
        for (int i = 0; i < count; i++) {
            if (values[i] != null && values[i].equals(value)) {
                n++;
            }
        }
        return n;
    }

    int fingerprint() {
        int h = 17;
        for (int i = 0; i < count; i++) {
            h = h * 31 + keys[i].hashCode();
            if (values[i] != null) {
                h = h * 31 + values[i].hashCode();
            }
        }
        return h;
    }

    public static void main(String[] args) {
        PropertyBag bag = new PropertyBag();
        bag.put("title", "Report");
        bag.put("pages", 12);
        bag.put("copies", "3");
        bag.put("ratio", 1.5);
        bag.put("color", true);
        bag.put("size", new Size(210, 297));
        bag.put("margin", new Size(20, 20));
        bag.put("extra", new Unset());
        bag.put("pages", 14);

        String[] names = { "title", "pages", "copies", "ratio", "color", "size", "extra", "missing" };
        for (String name : names) {
            System.out.println(bag.describe(name));
        }
        System.out.println("pages " + bag.getInt("pages", 0) + ", copies " + bag.getInt("copies", 1)
            + ", title " + bag.getInt("title", -1));
        System.out.println("color " + bag.getFlag("color") + ", title " + bag.getFlag("title"));
        System.out.println("A4 sizes " + bag.countEqual(new Size(210, 297)) + ", fourteen " + bag.countEqual(14)
            + ", text " + bag.countEqual("Report"));
        System.out.println("fingerprint " + bag.fingerprint());
        System.out.println("extra " + bag.get("extra"));
        Object size = bag.get("size");
        System.out.println("size " + size + " hash " + size.hashCode() + " same " + size.equals(new Size(210, 297)));
    }
}
