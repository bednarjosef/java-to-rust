public class Varargs {
    static int sum(int... xs) {
        int total = 0;
        for (int x : xs) {
            total += x;
        }
        return total;
    }

    static int count(String... words) {
        return words.length;
    }

    static String join(String sep, String... parts) {
        String out = "";
        for (int i = 0; i < parts.length; i++) {
            if (i > 0) {
                out = out + sep;
            }
            out = out + parts[i];
        }
        return out;
    }

    static void fill(int... xs) {
        for (int i = 0; i < xs.length; i++) {
            xs[i] = i * 10;
        }
    }

    static int[] keep(int... xs) {
        return xs;
    }

    static class Bag {
        final int size;

        Bag(int... items) {
            this.size = items.length;
        }
    }

    public static void main(String... args) {
        System.out.println(args.length);
        System.out.println(sum());
        System.out.println(sum(7));
        System.out.println(sum(1, 2, 3));
        int[] arr = {4, 5, 6};
        System.out.println(sum(arr));
        System.out.println(count());
        System.out.println(count("a", "b"));
        System.out.println(join("-", "x", "y", "z"));
        System.out.println(join("+"));
        String[] names = {"p", "q"};
        System.out.println(join(",", names));
        fill(arr);
        System.out.println(arr[0] + " " + arr[1] + " " + arr[2]);
        fill();
        System.out.println(sum(arr) + sum(1));
        Bag empty = new Bag();
        Bag full = new Bag(8, 9);
        System.out.println(empty.size + " " + full.size);
        int[] shared = {2, 3};
        int[] alias = keep(shared);
        shared[0] = 40;
        System.out.println(alias[0]);
        System.out.println(sum(new int[] {1, 1}));
        System.out.println(sum(null == null ? 2 : 3));
    }
}
