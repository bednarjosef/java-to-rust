import java.util.Arrays;

public class Prog {
    static String sStr = "\u00e9t\u00e9";
    public static void main(String[] args) {
        if (("a\\b".replace("ab", "abc").contains((sStr.split(";").length == 0 ? "" : sStr.split(";")[Math.floorMod(2, sStr.split(";").length)])))) {
        }
    }
}
