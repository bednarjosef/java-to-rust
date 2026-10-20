public class ParameterMutation {
    static class Counter {
        int count;
        String label;
        Counter(String label) { this.label = label; }
        void add(int n) { count += n; }
        public String toString() { return label + "=" + count; }
    }

    static void addTo(Counter c, int n) {
        c.add(n);
    }

    static void replace(Counter c) {
        c = new Counter("replaced");
        c.add(99);
    }

    static void swap(Counter a, Counter b) {
        Counter t = a;
        a = b;
        b = t;
        System.out.println("inside " + a + " " + b);
    }

    static void fill(int[] arr, int v) {
        for (int i = 0; i < arr.length; i++) arr[i] = v + i;
        arr = new int[] { 7, 7 };
        arr[0] = 8;
    }

    static Counter pick(Counter a, Counter b, boolean first) {
        return first ? a : b;
    }

    static int total(Counter[] cs) {
        int t = 0;
        for (Counter c : cs) {
            c.add(1);
            t += c.count;
        }
        return t;
    }

    public static void main(String[] args) {
        Counter c = new Counter("c");
        Counter d = c;
        addTo(c, 3);
        System.out.println(c + " " + d);
        replace(c);
        System.out.println(c);
        Counter e = new Counter("e");
        swap(c, e);
        System.out.println(c + " " + e);
        int[] nums = new int[3];
        int[] alias = nums;
        fill(nums, 5);
        System.out.println(nums[0] + " " + nums[1] + " " + nums[2] + " " + alias.length);
        Counter chosen = pick(c, e, false);
        chosen.add(1000);
        System.out.println(e + " " + c);
        Counter[] arr = { c, e, c };
        System.out.println(total(arr) + " " + c.count + " " + e.count);
        Counter n = null;
        System.out.println(pick(n, c, true) == null ? "none" : "some");
        System.out.println(pick(n, c, false).label);
    }
}
