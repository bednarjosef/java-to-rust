/**
 * Switch expressions nested 18 deep, the innermost with a loop, pattern variables and `new`s,
 * whose objects the stack-objects analysis (`analysis::escape`) places walking each level once.
 */
public class NestedSwitchExpressions {
    static String pick(int a, Object o) {
        return switch ((a >> 0) & 1) { case 0 -> switch ((a >> 1) & 1) { case 0 -> switch ((a >> 2) & 1) { case 0 -> switch ((a >> 3) & 1) { case 0 -> switch ((a >> 4) & 1) { case 0 -> switch ((a >> 5) & 1) { case 0 -> switch ((a >> 6) & 1) { case 0 -> switch ((a >> 7) & 1) { case 0 -> switch ((a >> 8) & 1) { case 0 -> switch ((a >> 9) & 1) { case 0 -> switch ((a >> 10) & 1) { case 0 -> switch ((a >> 11) & 1) { case 0 -> switch ((a >> 12) & 1) { case 0 -> switch ((a >> 13) & 1) { case 0 -> switch ((a >> 14) & 1) { case 0 -> switch ((a >> 15) & 1) { case 0 -> switch ((a >> 16) & 1) { case 0 -> switch ((a >> 17) & 1) { case 0 -> { StringBuilder b = new StringBuilder(); for (int i = 0; i < 3; i++) { if (o instanceof Integer n) { b.append(new StringBuilder().append(n + i)); } else if (o instanceof String s) { b.append(new StringBuilder(s).reverse()); } } yield b.toString(); } default -> o instanceof String t17 ? t17 + "17" : "level 17"; }; default -> o instanceof String t16 ? t16 + "16" : "level 16"; }; default -> o instanceof String t15 ? t15 + "15" : "level 15"; }; default -> o instanceof String t14 ? t14 + "14" : "level 14"; }; default -> o instanceof String t13 ? t13 + "13" : "level 13"; }; default -> o instanceof String t12 ? t12 + "12" : "level 12"; }; default -> o instanceof String t11 ? t11 + "11" : "level 11"; }; default -> o instanceof String t10 ? t10 + "10" : "level 10"; }; default -> o instanceof String t9 ? t9 + "9" : "level 9"; }; default -> o instanceof String t8 ? t8 + "8" : "level 8"; }; default -> o instanceof String t7 ? t7 + "7" : "level 7"; }; default -> o instanceof String t6 ? t6 + "6" : "level 6"; }; default -> o instanceof String t5 ? t5 + "5" : "level 5"; }; default -> o instanceof String t4 ? t4 + "4" : "level 4"; }; default -> o instanceof String t3 ? t3 + "3" : "level 3"; }; default -> o instanceof String t2 ? t2 + "2" : "level 2"; }; default -> o instanceof String t1 ? t1 + "1" : "level 1"; }; default -> o instanceof String t0 ? t0 + "0" : "level 0"; };
    }

    public static void main(String[] args) {
        System.out.println(pick(0, "ab"));
        System.out.println(pick(0, 7));
        System.out.println(pick(5, "cd"));
    }
}
