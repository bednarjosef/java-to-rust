public class ArrayStoreFromMethodCall {
    static class Machine {
        private final int[] memory = new int[4];
        private int sp = 2;

        int pop() {
            sp--;
            return sp;
        }

        void store(int slot) {
            memory[slot] = pop();
        }

        void storeIndexFromCall() {
            memory[pop()] = sp * 10;
        }

        void addFromCall() {
            memory[sp] += pop();
        }

        void stepFromCall() {
            memory[pop()]++;
        }
    }

    public static void main(String[] args) {
        Machine m = new Machine();
        m.store(1);
        System.out.println(m.memory[1]);
        m.storeIndexFromCall();
        System.out.println(m.memory[0] + " " + m.sp);
        m.addFromCall();
        System.out.println(m.memory[1] + " " + m.sp);
        m.stepFromCall();
        System.out.println(m.memory[0] + " " + m.sp);
    }
}
