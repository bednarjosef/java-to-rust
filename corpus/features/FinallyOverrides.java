// What a `finally` does itself replaces how its `try` ended: a `return` in it discards the
// value being returned or the exception being thrown, an exception thrown in it replaces the one
// on its way out, and an exception thrown by a `catch` clause passes through the `finally`
// (here also `java.io`'s exceptions, wrapped as causes and told apart by their class names).
public class FinallyOverrides {
    static class ParseError extends Exception {
        ParseError(String message) {
            super(message);
        }
    }

    static int calls = 0;

    @SuppressWarnings("finally")
    static int clamp(int n) {
        try {
            if (n >= 0) {
                return n;
            }
            throw new IllegalStateException("negative " + n);
        } finally {
            calls++;
            if (n > 5 || n < -5) {
                return 5;
            }
        }
    }

    static int parse(String s) throws ParseError {
        try {
            if (s.isEmpty()) {
                throw new ParseError("empty");
            }
            return s.length();
        } finally {
            System.out.println("parsed '" + s + "'");
        }
    }

    static int total(String[] parts) throws ParseError {
        int sum = 0;
        int i = 0;
        do {
            String part = parts[i];
            try {
                if (part.equals("skip")) {
                    continue;
                }
                sum += parse(part);
            } finally {
                i++;
                String note = "step " + i;
                System.out.println(note);
            }
        } while (i < parts.length);
        return sum;
    }

    static void rethrow(int n) throws ParseError {
        try {
            parse(n > 0 ? "x" : "");
        } catch (ParseError e) {
            System.out.println("rethrowing " + e.getMessage());
            throw new ParseError("again: " + e.getMessage());
        } finally {
            System.out.println("rethrow finally");
        }
    }

    static void cleanup(int n) {
        try {
            try {
                System.out.println("work " + n);
                if (n == 1) {
                    throw new IllegalStateException("work failed");
                }
            } finally {
                System.out.println("cleanup " + n);
                if (n == 2) {
                    throw new UnsupportedOperationException("cleanup failed");
                }
            }
        } catch (RuntimeException e) {
            System.out.println("handled " + e.getMessage());
        }
    }

    static int block(int n) {
        int r = 0;
        found:
        {
            try {
                if (n == 2) {
                    break found;
                }
                r = 10;
            } finally {
                r += 1;
            }
            r += 100;
        }
        return r;
    }

    static void save(String name) throws Exception {
        try {
            if (name.endsWith(".tmp")) {
                throw new java.io.FileNotFoundException(name);
            }
            throw new java.io.IOException("disk full");
        } catch (java.io.IOException e) {
            Exception wrapped = new Exception("cannot save " + name, e);
            throw wrapped;
        } finally {
            System.out.println("closed " + name);
        }
    }

    public static void main(String[] args) {
        for (String name : new String[] {"a.txt", "b.tmp"}) {
            try {
                save(name);
            } catch (Exception e) {
                Throwable cause = e.getCause();
                System.out.println(e.getClass().getSimpleName() + ": " + e.getMessage());
                if (cause != null) {
                    System.out.println("  " + cause.getClass().getSimpleName() + " " + cause.getClass().getName());
                }
            }
        }
        System.out.println(clamp(3));
        System.out.println(clamp(7));
        System.out.println(clamp(-9));
        try {
            System.out.println(clamp(-1));
        } catch (IllegalStateException e) {
            System.out.println("caught " + e.getMessage());
        }
        System.out.println("calls " + calls);
        try {
            System.out.println(total(new String[] {"ab", "skip", "abc"}));
            System.out.println(total(new String[] {"ab", "", "abc"}));
        } catch (ParseError e) {
            System.out.println("parse error " + e.getMessage());
        }
        try {
            rethrow(1);
            rethrow(0);
        } catch (ParseError e) {
            System.out.println("main " + e.getMessage());
        }
        cleanup(0);
        cleanup(1);
        cleanup(2);
        System.out.println(block(1) + " " + block(2));
        try {
            rethrow(0);
        } catch (ParseError e) {
            System.out.println("last " + e.getMessage());
            throw new IllegalStateException("gave up");
        } finally {
            System.out.println("the end");
        }
    }
}
