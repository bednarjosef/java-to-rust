public class NullableToStringVisited {
    interface Visitor {
        Object visitString(String s, Object p);
    }

    static class Err {
        final boolean flag;
        final String text;

        Err(boolean flag, String text) {
            this.flag = flag;
            this.text = text;
        }

        public String toString() {
            if (!flag) {
                return null;
            }
            return text.trim();
        }

        Object accept(Visitor v, Object p) {
            String s = toString();
            return v.visitString(s, p);
        }
    }

    public static void main(String[] args) {
        Visitor show = (s, p) -> s + " " + p;
        System.out.println(new Err(true, "  <error>  ").accept(show, "!"));
        System.out.println(new Err(false, "unused").accept(show, "?"));
        try {
            System.out.println(new Err(true, null).accept(show, "npe"));
        } catch (NullPointerException e) {
            System.out.println("caught");
        }
    }
}
