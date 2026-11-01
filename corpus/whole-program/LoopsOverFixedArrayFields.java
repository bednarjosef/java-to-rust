// Counting loops over an array field of `this` that only constructors assign (a `final` field):
// the field holds the same array all through a method, so `i < this.data.length` keeps
// `this.data[i]` in bounds, as it does for a local. Calls in the loop's body change elements but
// never the field. An index past the end still throws Java's ArrayIndexOutOfBoundsException, and
// a constructor's loop, which runs before the field may be assigned, keeps its checks.
import java.util.ArrayList;
import java.util.List;

public class LoopsOverFixedArrayFields {
    static final class Counters {
        private final int[] data;
        private final String[] names;
        private int calls;

        Counters(int n) {
            data = new int[n];
            names = new String[n];
            for (int i = 0; i < data.length; i++) {
                data[i] = i * 3 % 7;
                names[i] = "c" + i;
            }
        }

        private int bump(int i) {
            calls++;
            data[(i + 1) % data.length] += 1;
            return i;
        }

        int sum() {
            int s = 0;
            for (int i = 0; i < this.data.length; i++) {
                s += this.data[i] + bump(i);
            }
            return s;
        }

        void scale(int k) {
            for (int i = data.length - 1; i >= 0; i--) {
                data[i] = data[i] * k;
            }
        }

        String joined() {
            StringBuilder b = new StringBuilder();
            for (int i = 0; i < names.length; i++) {
                b.append(names[i]).append('=').append(data[i] % 10).append(' ');
            }
            return b.toString().trim();
        }

        int pastTheEnd() {
            int s = 0;
            for (int i = 0; i < data.length; i++) {
                s += data[i + 1];
            }
            return s;
        }
    }

    // Objects of this class reach another thread, so their fields are shared.
    static final class Shared implements Runnable {
        private final long[] values;
        private final List<String> log = new ArrayList<>();

        Shared(int n) {
            values = new long[n];
        }

        @Override
        public void run() {
            for (int round = 0; round < 3; round++) {
                for (int i = 0; i < values.length; i++) {
                    values[i] += i * round + 1;
                    note(i);
                }
            }
        }

        private void note(int i) {
            if (i % 4 == 0) {
                log.add("v" + i);
            }
        }

        long total() {
            long t = 0;
            for (int i = 0; i < values.length; i++) {
                t = t * 31 + values[i];
            }
            return t;
        }

        long pastTheEnd() {
            long t = 0;
            for (int i = 0; i <= values.length; i++) {
                t += values[i];
            }
            return t;
        }
    }

    public static void main(String[] args) throws Exception {
        Counters c = new Counters(9);
        System.out.println(c.sum() + " " + c.calls);
        c.scale(3);
        System.out.println(c.joined());
        try {
            System.out.println(c.pastTheEnd());
        } catch (ArrayIndexOutOfBoundsException e) {
            System.out.println("Counters " + e.getMessage());
        }
        System.out.println(new Counters(0).sum());

        Shared s = new Shared(10);
        Thread t = new Thread(s);
        t.start();
        t.join();
        System.out.println(s.total() + " " + s.log);
        try {
            System.out.println(s.pastTheEnd());
        } catch (ArrayIndexOutOfBoundsException e) {
            System.out.println("Shared " + e.getMessage());
        }
    }
}
