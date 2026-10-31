// A lambda and a method reference that capture nothing are one object per place they are written,
// whichever thread runs the place first: each worker gets them from the same places as the main
// thread and hands them back, and all are the same objects (as HotSpot makes them).
import java.util.Comparator;
import java.util.function.IntBinaryOperator;

public class LambdaPlacesAcrossThreads {
    static Comparator<String> byLength() {
        return (a, b) -> a.length() - b.length();
    }

    static IntBinaryOperator larger() {
        return Math::max;
    }

    public static void main(String[] args) throws InterruptedException {
        Comparator<String> mine = byLength();
        IntBinaryOperator myLarger = larger();
        Comparator<String>[] orders = new Comparator[3];
        IntBinaryOperator[] largers = new IntBinaryOperator[3];
        int[] compared = new int[3];
        Thread[] workers = new Thread[3];
        for (int w = 0; w < 3; w++) {
            int id = w;
            workers[w] = new Thread(() -> {
                Comparator<String> order = byLength();
                compared[id] = order.compare("pear", "kiwi" + id);
                orders[id] = order;
                largers[id] = larger();
            });
            workers[w].start();
        }
        for (Thread worker : workers) {
            worker.join();
        }
        for (int w = 0; w < 3; w++) {
            System.out.println(compared[w] + " " + (orders[w] == mine) + " "
                + (largers[w] == myLarger) + " " + largers[w].applyAsInt(w, 1));
        }
        System.out.println(orders[0] == orders[1] && orders[1] == orders[2]);
    }
}
