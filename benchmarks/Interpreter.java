// A small stack-machine interpreter: a fetch loop that dispatches on the opcode with a switch,
// pushes and pops an int stack, and reads and writes local variables.
public class Interpreter {
    static final int PUSH = 0;
    static final int ADD = 1;
    static final int SUB = 2;
    static final int MUL = 3;
    static final int DUP = 4;
    static final int LOAD = 5;
    static final int STORE = 6;
    static final int JNZ = 7;
    static final int HALT = 8;

    // Computes acc = acc * 3 + counter for counter from n down to 1, then halts with acc.
    static int[] program(int n) {
        return new int[] {
            PUSH, n, STORE, 0,
            PUSH, 0, STORE, 1,
            LOAD, 1, PUSH, 3, MUL, LOAD, 0, ADD, STORE, 1,
            LOAD, 0, PUSH, 1, SUB, DUP, STORE, 0,
            JNZ, 8,
            LOAD, 1, HALT
        };
    }

    static int run(int[] code) {
        int[] stack = new int[64];
        int[] vars = new int[4];
        int sp = 0;
        int pc = 0;
        while (true) {
            // Case labels are the opcode constants above, written out as literals.
            switch (code[pc]) {
                case 0: // PUSH
                    stack[sp++] = code[pc + 1];
                    pc += 2;
                    break;
                case 1: // ADD
                    sp--;
                    stack[sp - 1] += stack[sp];
                    pc++;
                    break;
                case 2: // SUB
                    sp--;
                    stack[sp - 1] -= stack[sp];
                    pc++;
                    break;
                case 3: // MUL
                    sp--;
                    stack[sp - 1] *= stack[sp];
                    pc++;
                    break;
                case 4: // DUP
                    stack[sp] = stack[sp - 1];
                    sp++;
                    pc++;
                    break;
                case 5: // LOAD
                    stack[sp++] = vars[code[pc + 1]];
                    pc += 2;
                    break;
                case 6: // STORE
                    vars[code[pc + 1]] = stack[--sp];
                    pc += 2;
                    break;
                case 7: // JNZ
                    if (stack[--sp] != 0) {
                        pc = code[pc + 1];
                    } else {
                        pc += 2;
                    }
                    break;
                case 8: // HALT
                    return stack[sp - 1];
                default:
                    return -1;
            }
        }
    }

    public static void main(String[] args) {
        int[] code = program(40000000);
        for (int warm = 0; warm < 2; warm++) {
            run(code);
        }
        long start = System.nanoTime();
        int result = run(code);
        long elapsed = (System.nanoTime() - start) / 1000000;
        System.out.println("interpreter result " + result);
        System.err.println("kernel-ms " + elapsed);
    }
}
