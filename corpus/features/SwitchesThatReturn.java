// Switch statements whose every case returns or throws, with a default: the code after them never
// runs, in a `try` as javac's Lower folds operators, and at the end of a method.
public class SwitchesThatReturn {
    enum Op {
        NEG,
        NOT,
        INC
    }

    static Integer fold(int opcode, int value) {
        try {
            switch (opcode) {
                case 1:
                    return -value;
                case 2:
                case 3:
                    return 10 / value;
                default:
                    return null;
            }
        } catch (ArithmeticException e) {
            return null;
        }
    }

    static String name(String s) {
        switch (s) {
            case "a" -> {
                return "first";
            }
            case "b" -> throw new IllegalArgumentException("b");
            default -> {
                return "other " + s;
            }
        }
    }

    static int apply(Op op, int x) {
        switch (op) {
            case NEG:
                return -x;
            case NOT:
                return ~x;
            default:
                if (x > 100) {
                    throw new IllegalStateException("big");
                }
                return x + 1;
        }
    }

    static int labeled(int x) {
        out:
        switch (x) {
            case 0:
                break out;
            default:
                return x * 2;
        }
        return -1;
    }

    public static void main(String[] args) {
        System.out.println(fold(1, 4) + " " + fold(2, 0) + " " + fold(3, 5) + " " + fold(9, 1));
        System.out.println(name("a") + " " + name("z"));
        try {
            name("b");
        } catch (IllegalArgumentException e) {
            System.out.println("caught " + e.getMessage());
        }
        System.out.println(apply(Op.NEG, 3) + " " + apply(Op.NOT, 3) + " " + apply(Op.INC, 3));
        System.out.println(labeled(0) + " " + labeled(4));
    }
}
