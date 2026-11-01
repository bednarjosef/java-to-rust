// Exceptions thrown several calls below a catch of Throwable, Exception, RuntimeException or
// Error, which the translation catches by unwinding: program exceptions, failures of the runtime
// (null, a null field's Java message, index, division, cast, size), their messages and causes; a clause that does not match
// (Exception around an Error) passing the exception on to an outer catch; and the frames on the
// way that hold a finally, a specific catch, a monitor and a resource, which still run.
public class CatchAllsFarFromTheThrow {
    static class Problem extends RuntimeException {
        final int code;

        Problem(String message, int code) {
            super(message);
            this.code = code;
        }
    }

    static class Checked extends Exception {
        Checked(String message, Throwable cause) {
            super(message, cause);
        }
    }

    static class Resource implements AutoCloseable {
        final String name;

        Resource(String name) {
            this.name = name;
            note("open " + name);
        }

        @Override
        public void close() {
            note("close " + name);
        }
    }

    static final StringBuilder log = new StringBuilder();
    static final Object lock = new Object();
    static void note(String line) {
        log.append(line).append('\n');
    }

    static class Node {
        Node next;
        int size = 3;
    }

    static Node chain = new Node();
    static int[] numbers = {1, 2, 3};
    static String nothing;
    static Object box = "text";

    static int leaf(int kind) {
        switch (kind) {
            case 0: return numbers[kind + 5];
            case 1: return nothing.length();
            case 2: return 10 / (kind - 2);
            case 3: return ((Integer) box) + 1;
            case 4: return new int[kind - 9].length;
            case 5: throw new Problem("problem " + kind, kind * 7);
            case 6: throw new IllegalStateException("state " + kind);
            case 7: throw new AssertionError("asserted " + kind);
            case 8: return chain.next.size;
            default: return kind;
        }
    }

    static int middle(int kind) {
        int value = leaf(kind) * 2;
        return value + 1;
    }

    // A frame with a finally between the throw and the catch.
    static int withFinally(int kind) {
        try {
            return middle(kind);
        } finally {
            note("finally " + kind);
        }
    }

    // A frame with a specific catch that matches one of the exceptions and not the others.
    static int withSpecificCatch(int kind) {
        try {
            return withFinally(kind);
        } catch (IllegalStateException e) {
            note("specific " + e.getMessage());
            return -1;
        }
    }

    // A frame that holds a monitor while the exception passes.
    static int withMonitor(int kind) {
        synchronized (lock) {
            return withSpecificCatch(kind);
        }
    }

    // A frame with a resource, closed while the exception passes.
    static int withResource(int kind) {
        try (Resource r = new Resource("r" + kind)) {
            return withMonitor(kind);
        }
    }

    static int top(int kind) {
        int result = withResource(kind);
        note("top " + kind + " returned " + result);
        return result;
    }

    static void checked(int kind) throws Checked {
        try {
            top(kind);
        } catch (RuntimeException e) {
            throw new Checked("wrapped", e);
        }
    }

    public static void main(String[] args) {
        for (int kind = 0; kind <= 9; kind++) {
            try {
                try {
                    System.out.println("value " + top(kind));
                } catch (Exception e) {
                    System.out.println("exception " + e);
                    if (e instanceof Problem p) {
                        System.out.println("code " + p.code);
                    }
                }
            } catch (Throwable t) {
                System.out.println("throwable " + t.getClass().getName() + ": " + t.getMessage());
            }
            System.out.println("holds lock " + Thread.holdsLock(lock));
        }
        System.out.print(log);
        log.setLength(0);
        try {
            checked(1);
        } catch (Checked e) {
            System.out.println(e.getMessage() + " " + e.getCause().getClass().getSimpleName());
        }
        try {
            checked(7);
        } catch (Error e) {
            System.out.println("error " + e.getMessage());
        } catch (Exception e) {
            System.out.println("wrong " + e);
        }
        System.out.println(log);
        int caught = 0;
        for (int i = 0; i < 1000; i++) {
            try {
                middle(i % 10);
            } catch (RuntimeException e) {
                caught++;
            } catch (Error e) {
                caught += 100;
            }
        }
        System.out.println("caught " + caught);
        middle(5);
    }
}
