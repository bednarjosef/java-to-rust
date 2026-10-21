// Try statements inside loops and switches and switches inside try statements: continue and
// break to labels from catch clauses, a return from a catch clause, a local assigned in a try
// and in its catch clause, nested trys in a catch clause, and a retry loop.
public class TryInControlFlow {
    static class Stop extends RuntimeException {
        final int at;

        Stop(int at) {
            super("stop at " + at);
            this.at = at;
        }
    }

    private int calls;

    int check(int x) {
        calls++;
        if (x % 7 == 6) {
            throw new Stop(x);
        }
        return x * 3;
    }

    static int sum(TryInControlFlow t, int[] xs) {
        int total = 0;
        loop:
        for (int x : xs) {
            switch (x % 3) {
                case 0:
                    try {
                        total += t.check(x);
                    } catch (Stop e) {
                        System.out.println("case 0 stopped " + e.at);
                        continue loop;
                    }
                    break;
                case 1:
                    try {
                        if (x > 20) {
                            break loop;
                        }
                        total -= t.check(x);
                    } catch (Stop e) {
                        return -e.at;
                    }
                    break;
                default:
                    total += x;
            }
        }
        return total;
    }

    static String describe(int code) {
        try {
            switch (code) {
                case 1:
                    return "one";
                case 2:
                    throw new IllegalArgumentException("two is not allowed");
                default:
                    break;
            }
            int len;
            try {
                len = code < 0 ? Integer.MAX_VALUE : code;
                if (len > 100) {
                    throw new IllegalStateException("too big " + len);
                }
            } catch (IllegalStateException e) {
                len = -1;
                System.out.println("inner " + e.getMessage());
            }
            return "len " + len;
        } catch (IllegalArgumentException e) {
            try {
                throw new UnsupportedOperationException(e.getMessage());
            } catch (UnsupportedOperationException u) {
                return "nested " + u.getMessage();
            }
        }
    }

    static int retries(int failures) {
        int attempt = 0;
        while (true) {
            try {
                attempt++;
                if (attempt <= failures) {
                    throw new IllegalStateException("attempt " + attempt + " failed");
                }
                return attempt;
            } catch (IllegalStateException e) {
                System.out.println(e.getMessage());
                if (attempt >= 3) {
                    return -attempt;
                }
            }
        }
    }

    public static void main(String[] args) {
        TryInControlFlow t = new TryInControlFlow();
        System.out.println(sum(t, new int[] { 1, 2, 3, 6, 9, 13, 4, 30 }));
        System.out.println(sum(t, new int[] { 3, 27, 22, 10 }));
        System.out.println(sum(t, new int[] { 3, 4, 7, 10, 13, 34, 9 }));
        System.out.println("calls " + t.calls);
        for (int code = -1; code <= 200; code += 67) {
            System.out.println(code + " " + describe(code));
        }
        System.out.println(describe(1) + " " + describe(2));
        System.out.println(retries(0) + " " + retries(2) + " " + retries(5));
    }
}
