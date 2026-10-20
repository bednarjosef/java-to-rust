public class StringConcatenation {
    public static void main(String[] args) {
        char a = 'a';
        int one = 1;
        System.out.println("" + 'a' + 1);
        System.out.println('a' + 1 + "");
        System.out.println('a' + 1);
        System.out.println("x" + a + one);
        System.out.println(a + one + "x");
        System.out.println("" + (char) (a + 1));
        System.out.println((char) (a + 1));
        char c = 'a';
        c++;
        System.out.println(c);
        c += 2;
        System.out.println(c);
        System.out.println(c + c);
        System.out.println("" + c + c);
        System.out.println("v=" + 1.0f + " " + 2.5 + " " + 'z' + " " + 10L + " " + true + " " + (byte) -3 + " " + (short) 7);
        System.out.println(1 + 2 + "3" + 4 + 5);
        System.out.println("" + Integer.MIN_VALUE + Long.MIN_VALUE);
        System.out.println("" + -0.0f + (1.0 / 0) + (0.0 / 0));
        String s = null;
        System.out.println("null? " + s);
        System.out.println('A' + "" + '\t' + "|");
        char nl = '\n';
        System.out.println("[" + nl + "]");
        System.out.println((int) '\0' + " " + (int) '￿');
        System.out.println('x' == 120);
        System.out.println(c > 'b' ? "big" : "small");
        String t = "";
        t += 'q';
        t += 3;
        t += 4.0f;
        t += 'r' + 1;
        System.out.println(t);
    }
}
