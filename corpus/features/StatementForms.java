// Statement shapes that the translation writes differently: a switch with one labelled case and
// nothing else (if let), case bodies in braces that end in break or return, a local returned on
// the last line of a function, and a labelled continue from inside a switch.
public class StatementForms {
    static int counter;

    static void single(int n) {
        switch (n) {
            case 3:
                System.out.println("three");
                break;
        }
    }

    static void singleEmptyDefault(int n) {
        switch (n) {
            case 4:
                System.out.println("four");
                break;
            default:
                break;
        }
    }

    static void singleWithDefault(int n) {
        switch (n) {
            case 5:
                System.out.println("five");
                break;
            default:
                System.out.println("other " + n);
        }
    }

    static void braced(int n) {
        switch (n) {
            case 1: {
                System.out.println("braced one");
                break;
            }
            case 2: {
                counter++;
                break;
            }
            default: {
                System.out.println("braced other");
            }
        }
    }

    static String bracedReturn(int n) {
        switch (n) {
            case 1: {
                return "first";
            }
            default: {
                return "rest";
            }
        }
    }

    static int twice(int n) {
        int r = n + n;
        return r;
    }

    static int pick(int n) {
        if (n > 0) {
            int big = n * 10;
            return big;
        }
        int small = -n;
        return small;
    }

    static String label(int n) {
        String s = n > 0 ? "pos" : "neg";
        return s;
    }

    static int labelledContinue(int rows) {
        int total = 0;
        outer:
        for (int i = 0; i < rows; i++) {
            for (int j = 0; j < rows; j++) {
                switch (j) {
                    case 1:
                        continue outer;
                    case 2:
                        break;
                    default:
                        total += 10;
                }
                total += 1;
            }
        }
        return total;
    }

    public static void main(String[] args) {
        single(3);
        single(4);
        singleEmptyDefault(4);
        singleEmptyDefault(9);
        singleWithDefault(5);
        singleWithDefault(6);
        braced(1);
        braced(2);
        braced(3);
        System.out.println(counter);
        System.out.println(bracedReturn(1) + " " + bracedReturn(2));
        System.out.println(twice(21) + " " + pick(4) + " " + pick(-6));
        System.out.println(label(1) + " " + label(-1));
        System.out.println(labelledContinue(2) + " " + labelledContinue(3));
    }
}
