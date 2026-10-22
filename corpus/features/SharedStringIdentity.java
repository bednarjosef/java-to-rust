// `==` on strings that only the objects decide: new strings, literals, interned strings, and the
// methods that return their receiver itself when nothing changes.
public class SharedStringIdentity {
    interface Named {
        String name();
    }

    static class Fixed implements Named {
        final String name;

        Fixed(String name) {
            this.name = name;
        }

        public String name() {
            return name;
        }

        @Override
        public String toString() {
            return name;
        }
    }

    static String same(String s) {
        return s;
    }

    public static void main(String[] args) {
        String lit = "text";
        String made = new String("text");
        System.out.println(made == lit);
        System.out.println(made.intern() == lit);
        System.out.println(same(made) == made);
        System.out.println(made.trim() == made);
        System.out.println(" text ".trim() == lit);
        System.out.println(made.substring(0) == made);
        System.out.println(made.substring(1) == made.substring(1));
        System.out.println(made.concat("") == made);
        System.out.println(made.toLowerCase() == made);
        System.out.println(made.replace('z', 'y') == made);
        System.out.println(made.split(",")[0] == made);
        System.out.println(made.split("x")[0] == made);
        System.out.println(made.repeat(1) == made);
        System.out.println(made.toString() == made);
        Object fixed = new Fixed(made);
        System.out.println(fixed.toString() == made);
        System.out.println(String.valueOf(fixed) == made);
        Named named = new Fixed(lit);
        System.out.println(named.name() == "text");
        System.out.println(("" + made) == made);
        Object held = made;
        System.out.println(held == made);
        System.out.println(held.toString() == made);
        String[] texts = {made, lit};
        System.out.println(texts[0] == texts[1]);
        System.out.println(texts[0] == made);
        String nothing = null;
        System.out.println(nothing == null);
        System.out.println(String.format("") == "");
        System.out.println(new StringBuilder().toString() == "");
    }
}
