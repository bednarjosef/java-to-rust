// A closure type with default methods whose values reach other threads: the closures are Arcs,
// and its extension trait is the same on them.
public class ClosureMethodsAcrossThreads {
    interface Step {
        int apply(int x);

        default Step then(Step next) {
            return x -> next.apply(apply(x));
        }

        default Step twice() {
            return then(this);
        }
    }

    public static void main(String[] args) throws InterruptedException {
        int k = 5;
        Step add = x -> x + k;
        Step chain = add.then(x -> x * 3).twice();
        int[] out = new int[3];
        Thread[] workers = new Thread[3];
        for (int w = 0; w < 3; w++) {
            int index = w;
            Step own = index == 0 ? chain : add.then(x -> x - index);
            workers[w] = new Thread(() -> out[index] = own.twice().apply(index + 1));
            workers[w].start();
        }
        for (Thread t : workers) {
            t.join();
        }
        for (int v : out) {
            System.out.println(v);
        }
    }
}
