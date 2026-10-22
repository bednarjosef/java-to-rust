// A thread ends the scopes of an object that the main thread shares: the loop's start reads a
// field of `this`, which the body changes, so the loop must not hold a borrow of the object.
public class ThreadedLoopBoundThroughBorrow {
    static class Code {
        int nextreg = 2;
        int ended;

        void endScope(int adr) {
            synchronized (this) {
                ended += adr;
                nextreg++;
            }
        }

        void endScopes(int prevNextReg) {
            for (int i = nextreg; i < prevNextReg; i++) endScope(i);
        }
    }

    static class Scoper extends Thread {
        private final Code code;
        private final int limit;

        Scoper(Code code, int limit) {
            this.code = code;
            this.limit = limit;
        }

        @Override
        public void run() {
            code.endScopes(limit);
        }
    }

    public static void main(String[] args) throws InterruptedException {
        Code code = new Code();
        Scoper scoper = new Scoper(code, 6);
        scoper.start();
        scoper.join();
        System.out.println(code.ended + " " + code.nextreg);
        Scoper again = new Scoper(code, 3);
        again.start();
        again.join();
        System.out.println(code.ended + " " + code.nextreg);
    }
}
