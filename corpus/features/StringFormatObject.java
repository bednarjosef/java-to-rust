import java.util.Arrays;

public class StringFormatObject {
    static String sStr = "😀x";
    static int[] aInt = new int[1];
    static char[] aChar = new char[2];
    public static void main(String[] args) {
        String s6 = sStr.substring(Math.floorMod((0), sStr.length() + 1)).trim();
        String s10 = (s6.split(",").length == 0 ? "" : s6.split(",")[Math.floorMod(aInt[Math.floorMod(7, aInt.length)], s6.split(",").length)]).trim();
        System.out.println(String.format("<%d|%s|%5.2f>", (0), (s10.split(",").length == 0 ? "" : s10.split(",")[Math.floorMod(aChar.length, s10.split(",").length)]), 0.0));
    }
}
