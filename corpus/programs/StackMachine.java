public class StackMachine {
    static final int MEMORY = 8;

    /** Factorial of 6 by a loop: memory 0 holds n, memory 1 holds the product. */
    static final String[] FACTORIAL = {
        "push 6", "store 0", "push 1", "store 1",
        "load 0", "jz 15",
        "load 1", "load 0", "mul", "store 1",
        "load 0", "push 1", "sub", "store 0",
        "jmp 4",
        "load 1", "print", "halt",
    };

    /** Fibonacci numbers below 100, printed as they are produced. */
    static final String[] FIBONACCI = {
        "push 0", "store 0", "push 1", "store 1",
        "load 1", "push 100", "sub", "jz 99",
        "load 0", "load 1", "dup", "store 0", "add", "store 1", "jmp 4",
        "halt",
    };

    /** Ends by popping an empty stack, which stops the machine with exit code 3. */
    static final String[] BROKEN = {
        "push 10", "push 4", "swap", "sub", "print", "pop", "pop",
    };

    static String opOf(String line) {
        String op = "";
        for (int i = 0; i < line.length() && line.charAt(i) != ' '; i++) {
            op = op + line.charAt(i);
        }
        return op;
    }

    static int argOf(String line) {
        int i = 0;
        while (i < line.length() && line.charAt(i) != ' ') {
            i++;
        }
        i++;
        boolean negative = false;
        if (i < line.length() && line.charAt(i) == '-') {
            negative = true;
            i++;
        }
        int value = 0;
        while (i < line.length() && line.charAt(i) >= '0' && line.charAt(i) <= '9') {
            value = value * 10 + (line.charAt(i) - '0');
            i++;
        }
        return negative ? -value : value;
    }

    static class Machine {
        private final int[] stack = new int[32];
        private final int[] memory = new int[MEMORY];
        private int sp;
        private int steps;

        private void push(int v) {
            stack[sp] = v;
            sp++;
        }

        private int pop(int pc) {
            if (sp == 0) {
                System.out.println("error: stack underflow at pc " + pc);
                System.exit(3);
            }
            sp--;
            return stack[sp];
        }

        int run(String[] program, int maxSteps) {
            int pc = 0;
            while (pc < program.length && steps < maxSteps) {
                String line = program[pc];
                String op = opOf(line);
                steps++;
                switch (op) {
                    case "push":
                        push(argOf(line));
                        break;
                    case "pop":
                        pop(pc);
                        break;
                    case "add":
                        push(pop(pc) + pop(pc));
                        break;
                    case "sub": {
                        int b = pop(pc);
                        int a = pop(pc);
                        push(a - b);
                        break;
                    }
                    case "mul":
                        push(pop(pc) * pop(pc));
                        break;
                    case "dup": {
                        int top = pop(pc);
                        push(top);
                        push(top);
                        break;
                    }
                    case "swap": {
                        int b = pop(pc);
                        int a = pop(pc);
                        push(b);
                        push(a);
                        break;
                    }
                    case "load":
                        push(memory[argOf(line)]);
                        break;
                    case "store":
                        int popped = pop(pc);
                        memory[argOf(line)] = popped;
                        break;
                    case "print":
                        System.out.println("out " + pop(pc));
                        break;
                    case "jz":
                        if (pop(pc) == 0) {
                            pc = argOf(line);
                            continue;
                        }
                        break;
                    case "jmp":
                        pc = argOf(line);
                        continue;
                    case "halt":
                        return steps;
                    default:
                        System.out.println("unknown op '" + op + "' at pc " + pc);
                        System.exit(2);
                }
                pc++;
            }
            return steps;
        }

        int depth() {
            return sp;
        }
    }

    public static void main(String[] args) {
        Machine m = new Machine();
        int used = m.run(FACTORIAL, 1000);
        System.out.println("factorial: " + used + " steps, stack depth " + m.depth());

        Machine fib = new Machine();
        System.out.println("fibonacci: " + fib.run(FIBONACCI, 1000) + " steps");

        Machine capped = new Machine();
        System.out.println("capped: " + capped.run(FACTORIAL, 10) + " steps, memory[1] = " + capped.memory[1]);

        System.out.println("negative literal: " + argOf("push -42"));

        Machine broken = new Machine();
        broken.run(BROKEN, 100);
        System.out.println("not reached");
    }
}
