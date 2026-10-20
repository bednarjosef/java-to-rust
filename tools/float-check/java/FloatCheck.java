import java.io.BufferedReader;
import java.io.BufferedWriter;
import java.io.IOException;
import java.io.InputStreamReader;
import java.io.OutputStreamWriter;

/**
 * Reference side of the float check: reads one bit pattern per line (hex) and prints
 * `hex TAB Float.toString(...)` (or `Double.toString` with the `double` argument).
 * Run as `java FloatCheck.java float|double < bits`.
 */
public class FloatCheck {
    public static void main(String[] args) throws IOException {
        boolean isFloat = args[0].equals("float");
        BufferedReader in = new BufferedReader(new InputStreamReader(System.in), 1 << 16);
        BufferedWriter out = new BufferedWriter(new OutputStreamWriter(System.out), 1 << 16);
        String line;
        while ((line = in.readLine()) != null) {
            String value = isFloat
                ? Float.toString(Float.intBitsToFloat(Integer.parseUnsignedInt(line, 16)))
                : Double.toString(Double.longBitsToDouble(Long.parseUnsignedLong(line, 16)));
            out.write(line);
            out.write('\t');
            out.write(value);
            out.newLine();
        }
        out.flush();
    }
}
