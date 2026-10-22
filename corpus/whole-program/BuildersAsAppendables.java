import java.io.IOException;
import java.io.StringWriter;

// Builders and writers passed as `Appendable`s: each `append` form, a null text, a builder found
// again through `instanceof` and a cast, and a comparison with one.
public class BuildersAsAppendables {
    static Appendable fill(Appendable a, int n) throws IOException {
        a.append("n=").append(Integer.toString(n)).append(',');
        a.append("abcdef", 1, 4);
        a.append(null);
        return a;
    }

    static String describe(Appendable a) {
        if (a instanceof StringBuilder sb) {
            return "a builder of " + sb.length();
        }
        return "something else";
    }

    public static void main(String[] args) throws IOException {
        StringBuilder sb = new StringBuilder("<");
        Appendable same = fill(sb, 7);
        System.out.println(sb + " " + (same == sb));
        StringWriter writer = new StringWriter();
        fill(writer, 8);
        System.out.println(writer);
        System.out.println(describe(sb) + " / " + describe(writer));
        StringBuilder back = (StringBuilder) same;
        back.append('>');
        System.out.println(back);
    }
}
