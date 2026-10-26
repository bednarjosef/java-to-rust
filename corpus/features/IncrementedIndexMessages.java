// The messages of NullPointerExceptions of an element whose index increments or decrements a
// local as it is read (`a[i++]`, `a[++i]`): javac loads the local before or after its update,
// so the JVM names the index by the local.
public class IncrementedIndexMessages {
    static class State {
        int weight;

        State(int weight) {
            this.weight = weight;
        }

        int block() {
            return weight;
        }
    }

    static int update(int[] push, State[] states) {
        int index = 0;
        int total = 0;
        for (int i = push.length - 1; i >= 0; i--) {
            total += push[i] + states[index++].block();
        }
        return total;
    }

    static int ahead(State[] states) {
        int index = -1;
        int total = 0;
        while (index < states.length - 1) {
            total += states[++index].weight;
        }
        return total;
    }

    static int back(State[] states) {
        int index = states.length - 1;
        int total = 0;
        while (index >= 0) {
            total += states[index--].block();
        }
        return total;
    }

    interface Action {
        void run();
    }

    static void report(Action action) {
        try {
            action.run();
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
    }

    public static void main(String[] args) {
        State[] full = { new State(1), new State(2), new State(3) };
        State[] holes = { new State(1), null, new State(3) };
        int[] push = { 10, 20, 30 };
        report(() -> System.out.println(update(push, full)));
        report(() -> System.out.println(update(push, holes)));
        report(() -> System.out.println(ahead(full)));
        report(() -> System.out.println(ahead(holes)));
        report(() -> System.out.println(back(full)));
        report(() -> System.out.println(back(holes)));
    }
}
