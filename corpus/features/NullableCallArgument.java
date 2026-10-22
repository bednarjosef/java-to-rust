public class NullableCallArgument {
    static class Printer {
        String name = "printer";

        void rename(String name) {
            this.name = name;
        }

        String visitString(String s, String p) {
            return name + " " + (s == null ? "null" : s) + " " + p;
        }
    }

    static class Err {
        final boolean flag;
        final String text;
        final Printer printer;

        Err(boolean flag, String text, Printer printer) {
            this.flag = flag;
            this.text = text;
            this.printer = printer;
        }

        @Override
        public String toString() {
            System.out.println("toString");
            if (!flag) {
                return null;
            }
            return text.trim();
        }

        String accept(String p) {
            return printer.visitString(toString(), p);
        }
    }

    public static void main(String[] args) {
        Printer quiet = new Printer();
        quiet.rename("first");
        System.out.println(new Err(true, "  abc ", quiet).accept("x"));
        System.out.println(new Err(false, "abc", quiet).accept("y"));
        try {
            System.out.println(new Err(true, null, quiet).accept("w"));
        } catch (NullPointerException e) {
            System.out.println("npe");
        }
    }
}
