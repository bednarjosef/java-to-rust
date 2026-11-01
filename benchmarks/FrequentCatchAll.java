// The cost of a catch-all that often catches: items parsed by a helper inside a catch of
// Exception, a third of which are not numbers, so that the parse throws two calls below the
// catch.
public class FrequentCatchAll {
    static int parse(String text) {
        return Integer.parseInt(text.trim());
    }

    static int valueOf(String text) {
        return parse(text) * 2;
    }

    static long run(String[] items, int rounds) {
        long sum = 0;
        int failed = 0;
        for (int r = 0; r < rounds; r++) {
            for (String item : items) {
                try {
                    sum += valueOf(item);
                } catch (Exception e) {
                    failed++;
                }
            }
        }
        return sum * 1000 + failed;
    }

    public static void main(String[] args) {
        String[] items = new String[300];
        for (int i = 0; i < items.length; i++) {
            items[i] = i % 3 == 0 ? "item" + i : " " + i + " ";
        }
        long check = run(items, 100);
        long start = System.nanoTime();
        long result = run(items, 1000);
        long elapsed = (System.nanoTime() - start) / 1000000;
        System.out.println("frequent catch-all " + result + " " + check);
        System.err.println("kernel-ms " + elapsed);
    }
}
