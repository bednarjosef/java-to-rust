// A `yield` leaves its switch expression after the `finally` of every `try` it leaves has run.
// The value is computed before that `finally`, and a `finally` that changes the variable does
// not change the value yielded.
public class YieldThroughFinally {
    static int pick(int n) {
        return switch (n) {
            case 1 -> {
                int v = 1;
                try {
                    yield v;
                } finally {
                    v = 99;
                    System.out.println("finally 1 sees " + v);
                }
            }
            case 2 -> {
                int r;
                try {
                    if (n > 0) {
                        yield 20;
                    }
                    r = 0;
                } finally {
                    System.out.println("finally 2");
                }
                yield r;
            }
            case 3 -> {
                try {
                    try {
                        yield 30;
                    } finally {
                        System.out.println("inner");
                    }
                } finally {
                    System.out.println("outer");
                }
            }
            default -> {
                int total = 0;
                for (int i = 0; i < 5; i++) {
                    try {
                        if (i == n) {
                            yield total * 100;
                        }
                        total += i;
                    } finally {
                        System.out.println("round " + i);
                    }
                }
                yield -1;
            }
        };
    }

    static String describe(String word) {
        return switch (word.length()) {
            case 0 -> "empty";
            default -> {
                try {
                    if (word.startsWith("x")) {
                        yield "x-word";
                    }
                    yield "word of " + word.length();
                } catch (RuntimeException e) {
                    yield "unreachable";
                } finally {
                    System.out.println("described " + word);
                }
            }
        };
    }

    public static void main(String[] args) {
        System.out.println(pick(1));
        System.out.println(pick(2));
        System.out.println(pick(3));
        System.out.println(pick(3));
        System.out.println(pick(9));
        System.out.println(pick(2) + 1);
        System.out.println(describe(""));
        System.out.println(describe("xy"));
        System.out.println(describe("abc"));
    }
}
