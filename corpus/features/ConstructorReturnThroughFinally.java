// A `return` in a constructor runs the `finally` of every `try` it leaves, then the constructor
// ends, so the object is complete only if the constructor did not return early.
public class ConstructorReturnThroughFinally {
    static class Box {
        int value;
        String note = "none";

        Box(int n) {
            try {
                if (n < 0) {
                    note = "negative";
                    return;
                }
                note = "positive";
            } finally {
                System.out.println("finally for " + n);
            }
            value = n * 2;
        }
    }

    static class Counter {
        int count;

        Counter(int start) {
            count = start;
            for (int i = 0; i < 3; i++) {
                try {
                    try {
                        if (count > 10) {
                            return;
                        }
                        count += i;
                    } finally {
                        System.out.println("inner " + i + " count " + count);
                    }
                } finally {
                    count++;
                }
            }
            count *= 10;
        }
    }

    public static void main(String[] args) {
        Box a = new Box(4);
        System.out.println(a.value + " " + a.note);
        Box b = new Box(-1);
        System.out.println(b.value + " " + b.note);
        System.out.println(new Counter(0).count);
        System.out.println(new Counter(20).count);
    }
}
