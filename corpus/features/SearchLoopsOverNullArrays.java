// Search loops (`for (i = 0; i < a.length; i++) if (a[i] == key) return i;`) over arrays held in
// local variables that may be null, where a `catch` sees the NullPointerException of reading the
// length.
public class SearchLoopsOverNullArrays {
    final Object[] items;
    final int[] numbers;

    SearchLoopsOverNullArrays(Object[] items, int[] numbers) {
        this.items = items;
        this.numbers = numbers;
    }

    int indexOfNull() {
        Object[] a = this.items;
        for (int i = 0; i < a.length; i++) {
            if (a[i] == null) {
                return i;
            }
        }
        return -1;
    }

    int indexOf(Object o) {
        Object[] a = this.items;
        if (o == null) {
            for (int i = 0; i < a.length; i++) {
                if (a[i] == null) {
                    return i;
                }
            }
        } else {
            for (int i = 0; i < a.length; i++) {
                if (o.equals(a[i])) {
                    return i;
                }
            }
        }
        return -1;
    }

    int indexOfNumber(int n) {
        int[] a = this.numbers;
        for (int i = 0; i < a.length; i++) {
            if (a[i] == n) {
                return i;
            }
        }
        return -1;
    }

    public static void main(String[] args) {
        SearchLoopsOverNullArrays full = new SearchLoopsOverNullArrays(new Object[] {"x", null, "y"}, new int[] {4, 5, 6});
        System.out.println(full.indexOfNull() + " " + full.indexOf(null) + " " + full.indexOf("y") + " " + full.indexOfNumber(6));
        SearchLoopsOverNullArrays empty = new SearchLoopsOverNullArrays(null, null);
        try {
            System.out.println(empty.indexOf(null));
        } catch (NullPointerException e) {
            System.out.println("indexOf(null): NullPointerException");
        }
        try {
            System.out.println(empty.indexOfNull());
        } catch (NullPointerException e) {
            System.out.println("indexOfNull: NullPointerException");
        }
        try {
            System.out.println(empty.indexOfNumber(1));
        } catch (NullPointerException e) {
            System.out.println("indexOfNumber: NullPointerException");
        }
    }
}
