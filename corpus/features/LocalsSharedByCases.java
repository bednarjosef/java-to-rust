// The cases of a switch block share one scope: a local that one case declares is assigned and
// used in a later one, as the JDK's UTF-8 decoder does in `malformedN`.
public class LocalsSharedByCases {
    static int measure(int kind, int[] bytes) {
        switch (kind) {
            case 1:
            case 2:
                return 1;
            case 3:
                int first = bytes[0];
                int second = bytes[1];
                return first == second ? 1 : 2;
            case 4:
                first = bytes[0] & 0xff;
                second = bytes[1] & 0xff;
                second++;
                if (first > second) {
                    return 3;
                }
                return first + second;
            default:
                return -1;
        }
    }

    static String describe(int n) {
        StringBuilder sb = new StringBuilder();
        switch (n) {
            case 0:
                String word = "zero";
                sb.append(word);
                break;
            case 1:
                word = "one";
                sb.append(word);
            case 2:
                word = "falls";
                sb.append(' ').append(word);
                break;
            default:
                sb.append("many");
        }
        return sb.toString();
    }

    public static void main(String[] args) {
        int[] bytes = {200, 7};
        for (int kind = 0; kind <= 5; kind++) {
            System.out.println(kind + " " + measure(kind, bytes));
        }
        for (int n = 0; n < 4; n++) {
            System.out.println(describe(n));
        }
    }
}
