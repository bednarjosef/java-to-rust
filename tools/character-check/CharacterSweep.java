import java.io.PrintStream;

/**
 * Prints what the JDK's Character methods return for every char 0..0xFFFF, one line per char,
 * and what Character.forDigit returns for small digits and radixes. The Rust program in rust/
 * prints the same lines through the runtime; check.sh diffs the two.
 *
 * Usage: java CharacterSweep.java chars | java CharacterSweep.java for-digit
 */
public class CharacterSweep {
    static final String CHARS_HEADER = "# code isDigit isLetter isLetterOrDigit isAlphabetic isUpperCase"
            + " isLowerCase isWhitespace isSpaceChar isSurrogate isHighSurrogate isLowSurrogate"
            + " toUpperCase toLowerCase getNumericValue"
            + " digit2 digit3 digit4 digit5 digit6 digit7 digit8 digit9 digit10 digit11 digit12"
            + " digit13 digit14 digit15 digit16 digit17 digit18 digit19 digit20 digit21 digit22"
            + " digit23 digit24 digit25 digit26 digit27 digit28 digit29 digit30 digit31 digit32"
            + " digit33 digit34 digit35 digit36";

    public static void main(String[] args) {
        PrintStream out = System.out;
        if (args.length == 1 && args[0].equals("chars")) {
            out.println(CHARS_HEADER);
            for (int c = 0; c <= 0xFFFF; c++) {
                printChar(out, (char) c);
            }
        } else if (args.length == 1 && args[0].equals("for-digit")) {
            out.println("# digit radix forDigit");
            for (int digit = -2; digit <= 37; digit++) {
                for (int radix = -2; radix <= 38; radix++) {
                    out.println(digit + " " + radix + " " + (int) Character.forDigit(digit, radix));
                }
            }
        } else {
            System.err.println("usage: CharacterSweep.java chars | for-digit");
            System.exit(2);
        }
    }

    static void printChar(PrintStream out, char c) {
        StringBuilder line = new StringBuilder();
        line.append(String.format("%04X", (int) c));
        line.append(bit(Character.isDigit(c)));
        line.append(bit(Character.isLetter(c)));
        line.append(bit(Character.isLetterOrDigit(c)));
        line.append(bit(Character.isAlphabetic(c)));
        line.append(bit(Character.isUpperCase(c)));
        line.append(bit(Character.isLowerCase(c)));
        line.append(bit(Character.isWhitespace(c)));
        line.append(bit(Character.isSpaceChar(c)));
        line.append(bit(Character.isSurrogate(c)));
        line.append(bit(Character.isHighSurrogate(c)));
        line.append(bit(Character.isLowSurrogate(c)));
        line.append(String.format(" %04X %04X", (int) Character.toUpperCase(c), (int) Character.toLowerCase(c)));
        line.append(" ").append(Character.getNumericValue(c));
        for (int radix = 2; radix <= 36; radix++) {
            line.append(" ").append(Character.digit(c, radix));
        }
        out.println(line);
    }

    static String bit(boolean b) {
        return b ? " 1" : " 0";
    }
}
