// Throws whose message is made by calls that can throw themselves (a lookup that fails), and
// messages with quotes in them and an object's toString(). The making runs out of line; an
// exception thrown while the message is made is the one the throw throws, as in Java, inside a
// try or not.
public class ThrowsWhileMakingTheMessage {
    static class Unbound extends RuntimeException {
        Unbound(String message) {
            super(message);
        }
    }

    static class Refused extends Exception {
        Refused(String message) {
            super(message);
        }
    }

    static class Key {
        final String name;

        Key(String name) {
            this.name = name;
        }

        @Override
        public String toString() {
            return name.isEmpty() ? "a key without a name" : "key '" + name + "'";
        }
    }

    static class Holder {
        final Object key;
        final String owner;
        Object value;

        Holder(Key key, String owner) {
            this.key = key;
            this.owner = owner;
        }

        Object value() {
            if (value == null) {
                throw new Unbound("Trying to access unbound value '" + key + "' from registry " + owner);
            }
            return value;
        }
    }

    static int lookups;

    static String describe(int code) throws Exception {
        lookups++;
        if (code < 0) {
            throw new Exception("no description of " + code);
        }
        return "code " + code;
    }

    static int checked(int code) throws Exception {
        if (code > 2) {
            throw new Refused("can't take " + describe(code) + ", it's above 2");
        }
        if (code < 0) {
            throw new Refused("can't take " + describe(code));
        }
        return code * 10;
    }

    static int caught(int code) {
        try {
            if (code > 1) {
                throw new Refused("'" + describe(code) + "' inside a try");
            }
            return code;
        } catch (Exception e) {
            return -100 - e.getMessage().length();
        }
    }

    public static void main(String[] args) {
        Holder bound = new Holder(new Key("stone"), "blocks");
        bound.value = "a stone";
        Holder[] holders = {bound, new Holder(new Key("dirt"), "blocks"), new Holder(new Key(""), "items")};
        for (Holder h : holders) {
            try {
                System.out.println("value " + h.value());
            } catch (RuntimeException e) {
                System.out.println(e.getClass().getSimpleName() + ": " + e.getMessage());
            }
        }
        long sum = 0;
        for (int i = 0; i < 1000; i++) {
            sum += bound.value().hashCode() & 7;
        }
        System.out.println("sum " + sum);
        for (int code = -1; code < 5; code++) {
            try {
                System.out.println("checked " + checked(code));
            } catch (Exception e) {
                System.out.println(e.getClass().getSimpleName() + ": " + e.getMessage());
            }
            System.out.println("caught " + caught(code));
        }
        System.out.println("lookups " + lookups);
    }
}
