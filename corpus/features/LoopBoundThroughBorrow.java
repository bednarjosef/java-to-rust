public class LoopBoundThroughBorrow {
    // Keeps the objects of `Code` shared, so that their fields are borrowed.
    static Code last;

    static class Code {
        int nextreg = 2;
        int ended;

        void endScope(int adr) {
            ended += adr;
            nextreg++;
            last = this;
        }

        // The loop's start reads a field of `this`, which the body changes: Java reads it once.
        void endScopes(int prevNextReg) {
            for (int i = nextreg; i < prevNextReg; i++) endScope(i);
        }
    }

    public static void main(String[] args) {
        Code code = new Code();
        code.endScopes(6);
        System.out.println(code.ended + " " + code.nextreg + " " + (last == code));
        code.endScopes(3);
        System.out.println(last.ended + " " + last.nextreg);
    }
}
