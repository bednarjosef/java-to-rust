public class CharArithmetic {
    static char shift(char c, int by) { return (char) (c + by); }
    public static void main(String[] args) {
        char c = 'x';
        System.out.println(c + 1);
        System.out.println((char) (c + 1));
        System.out.println(c - 'a');
        System.out.println((char) ('a' + (c - 'a' + 5) % 26));
        char d = '￿';
        d++;
        System.out.println((int) d);
        char e = 0;
        e--;
        System.out.println((int) e);
        System.out.println(e == '￿');
        char f = 'A';
        f += 32;
        System.out.println(f);
        f *= 2;
        System.out.println((int) f);
        System.out.println(shift('a', 2));
        System.out.println(shift('a', -98));
        char[] letters = new char[3];
        for (int i = 0; i < letters.length; i++) {
            letters[i] = (char) ('p' + i);
        }
        System.out.println(letters[0]);
        System.out.println(letters[2] - letters[0]);
        System.out.println("" + letters[1] + letters[1]);
        System.out.println(letters[1] + letters[1]);
        int sum = 0;
        for (char q = 'a'; q <= 'e'; q++) {
            sum += q;
        }
        System.out.println(sum);
        char big = (char) 0x1F600;
        System.out.println((int) big);
        System.out.println('é');
        System.out.println('é' + 0);
        System.out.println((char) 0x263A);
        short sh = 'z';
        System.out.println(sh);
        char fromShort = (char) (short) -2;
        System.out.println((int) fromShort);
        System.out.println('a' < 'b' && 'Z' < 'a');
        System.out.println(c == 120 ? "yes" : "no");
        char tab = '\t';
        System.out.println((int) tab + tab);
        System.out.println(Character.getNumericValue('7') + 1);
        System.out.println(Character.isDigit('7') ? 'd' : 'n');
    }
}
