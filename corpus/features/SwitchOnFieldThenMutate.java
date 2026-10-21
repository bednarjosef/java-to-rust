public class SwitchOnFieldThenMutate {
    static class Machine {
        int state;
        int moves;

        void moveTo(int next) {
            state = next;
            moves++;
        }
    }

    static void step(Machine m) {
        switch (m.state) {
            case 0:
                m.moveTo(1);
                break;
            case 1:
                m.moveTo(2);
                break;
            default:
                break;
        }
    }

    public static void main(String[] args) {
        Machine m = new Machine();
        step(m);
        step(m);
        step(m);
        System.out.println(m.state + " " + m.moves);
    }
}
