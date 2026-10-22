// Names that snake case turns into Rust's keywords, as javac's TreeMaker has (`Return`, `If`).
public class RustKeywordNames {
    String Enum = "enum";
    int Static = 3;

    static String Return(String s) {
        return "return " + s;
    }

    static int If(boolean b) {
        return b ? 1 : 0;
    }

    String Super() {
        return "super " + Enum;
    }

    static int Break(int[] a) {
        int Loop = 0;
        for (int While : a) {
            Loop += While;
        }
        return Loop;
    }

    public static void main(String[] args) {
        RustKeywordNames r = new RustKeywordNames();
        System.out.println(Return("x") + " " + If(true) + " " + r.Super() + " " + r.Static);
        System.out.println(Break(new int[] {1, 2, 3}));
    }
}
