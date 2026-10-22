public class NullReceiverArrayParams {
    int total;

    void add(int[] values, char[][] grid) {
        for (int v : values) {
            total += v;
        }
        total += grid.length;
    }

    static NullReceiverArrayParams pick(int i) {
        return i == 0 ? null : new NullReceiverArrayParams();
    }

    static StringBuilder builder(int i) {
        return i == 0 ? null : new StringBuilder("ab");
    }

    public static void main(String[] args) {
        for (int i = 0; i < 2; i++) {
            NullReceiverArrayParams p = pick(i);
            try {
                p.add(new int[] {1, 2}, new char[3][]);
                System.out.println(p.total);
            } catch (NullPointerException e) {
                System.out.println(e.getMessage());
            }
            StringBuilder sb = builder(i);
            try {
                sb.insert(1, new char[] {'x', 'y'});
                System.out.println(sb);
            } catch (NullPointerException e) {
                System.out.println(e.getMessage());
            }
        }
    }
}
