package app;

/**
 * Calls of the instance methods of a class that the program never creates an object of, as
 * java.nio's buffers check a memory session that only foreign memory creates: the receiver is
 * always null, so the body never runs and what it uses is not translated (here method handles,
 * which are reflection).
 */
public class Main {
    abstract static class Session {
        static final java.lang.invoke.MethodHandles.Lookup LOOKUP = java.lang.invoke.MethodHandles.lookup();

        abstract boolean closed();

        void check() {
            if (closed()) {
                throw new IllegalStateException("closed by " + LOOKUP);
            }
        }
    }

    static class Buffer {
        final Session session;
        final int[] data;

        Buffer(int size) {
            this.session = null;
            this.data = new int[size];
        }

        int get(int i) {
            if (session != null) {
                session.check();
            }
            return data[i];
        }
    }

    static int checked(Session session) {
        try {
            session.check();
            return 1;
        } catch (NullPointerException e) {
            return 0;
        }
    }

    public static void main(String[] args) {
        Buffer buffer = new Buffer(3);
        buffer.data[1] = 7;
        System.out.println(buffer.get(1));
        System.out.println(checked(buffer.session));
    }
}
