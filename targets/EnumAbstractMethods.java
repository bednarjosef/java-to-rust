// Features: enums with fields, constructors and abstract methods overridden per constant (constant
// bodies), a static lookup method over values(), valueOf with IllegalArgumentException, ordinal,
// compareTo, name, EnumMap and EnumSet, enum switch statements with fall-through, an enum
// implementing an interface, a static nested enum used as a HashMap key, and an enum-driven
// stack calculator.
import java.util.ArrayDeque;
import java.util.Deque;
import java.util.EnumMap;
import java.util.EnumSet;
import java.util.Map;

public class EnumAbstractMethods {
    interface Priced {
        int cents();
    }

    enum Operation implements Priced {
        ADD("+", 1) {
            @Override
            int apply(int a, int b) {
                return a + b;
            }
        },
        SUB("-", 1) {
            @Override
            int apply(int a, int b) {
                return a - b;
            }
        },
        MUL("*", 2) {
            @Override
            int apply(int a, int b) {
                return a * b;
            }
        },
        DIV("/", 3) {
            @Override
            int apply(int a, int b) {
                if (b == 0) {
                    throw new ArithmeticException("division by zero in " + name());
                }
                return a / b;
            }
        },
        POW("^", 4) {
            @Override
            int apply(int a, int b) {
                int result = 1;
                for (int i = 0; i < b; i++) {
                    result *= a;
                }
                return result;
            }

            @Override
            public int cents() {
                return 250;
            }
        };

        private final String symbol;
        private final int precedence;

        Operation(String symbol, int precedence) {
            this.symbol = symbol;
            this.precedence = precedence;
        }

        abstract int apply(int a, int b);

        public int cents() {
            return 100 * precedence;
        }

        String symbol() {
            return symbol;
        }

        static Operation fromSymbol(String symbol) {
            for (Operation op : values()) {
                if (op.symbol.equals(symbol)) {
                    return op;
                }
            }
            throw new IllegalArgumentException("unknown operator " + symbol);
        }
    }

    enum Day {
        MON, TUE, WED, THU, FRI, SAT, SUN;

        boolean weekend() {
            return this == SAT || this == SUN;
        }
    }

    static String kind(Day day) {
        String result;
        switch (day) {
            case SAT:
            case SUN:
                result = "rest";
                break;
            case FRI:
                result = "almost";
            default:
                result = "work";
        }
        return result;
    }

    static int evaluateRpn(String expression) {
        Deque<Integer> stack = new ArrayDeque<>();
        for (String token : expression.trim().split(" +")) {
            if (token.matches("-?\\d+")) {
                stack.push(Integer.parseInt(token));
            } else {
                Operation op = Operation.fromSymbol(token);
                int b = stack.pop();
                int a = stack.pop();
                stack.push(op.apply(a, b));
            }
        }
        return stack.pop();
    }

    public static void main(String[] args) {
        for (Operation op : Operation.values()) {
            System.out.println(op + " " + op.symbol() + " ordinal " + op.ordinal() + " apply(7, 3) = " + op.apply(7, 3)
                    + " cents " + op.cents());
        }
        System.out.println("valueOf MUL: " + Operation.valueOf("MUL").apply(6, 7));
        try {
            Operation.valueOf("MOD");
        } catch (IllegalArgumentException e) {
            System.out.println("valueOf failed: " + e.getMessage());
        }
        System.out.println("compare DIV to ADD: " + Operation.DIV.compareTo(Operation.ADD));
        System.out.println("declaring class is enum: " + Operation.POW.getDeclaringClass().isEnum());

        System.out.println("rpn 3 4 + 2 * = " + evaluateRpn("3 4 + 2 *"));
        System.out.println("rpn 2 10 ^ 1 - = " + evaluateRpn("2 10 ^ 1 -"));
        try {
            evaluateRpn("8 0 /");
        } catch (ArithmeticException e) {
            System.out.println("rpn error: " + e.getMessage());
        }

        EnumMap<Day, Integer> hours = new EnumMap<>(Day.class);
        hours.put(Day.WED, 6);
        hours.put(Day.MON, 8);
        hours.put(Day.SAT, 2);
        hours.merge(Day.MON, 1, Integer::sum);
        System.out.println("hours: " + hours);
        int total = 0;
        for (Map.Entry<Day, Integer> e : hours.entrySet()) {
            total += e.getValue();
        }
        System.out.println("total hours: " + total);

        EnumSet<Day> weekend = EnumSet.noneOf(Day.class);
        for (Day d : Day.values()) {
            if (d.weekend()) {
                weekend.add(d);
            }
        }
        System.out.println("weekend: " + weekend + " complement: " + EnumSet.complementOf(weekend));
        System.out.println("range: " + EnumSet.range(Day.TUE, Day.THU));
        for (Day d : new Day[] {Day.FRI, Day.SAT, Day.TUE}) {
            System.out.println(d + " is " + kind(d));
        }

        Map<Operation, Integer> usage = new EnumMap<>(Operation.class);
        usage.put(Operation.MUL, 2);
        usage.put(Operation.ADD, 5);
        System.out.println("usage: " + usage + " pow priced " + Operation.POW.cents());
    }
}
