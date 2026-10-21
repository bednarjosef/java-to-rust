import java.util.function.IntBinaryOperator;
import java.util.function.IntPredicate;

// Locals that hold a lambda only to call it: plain Rust closures that borrow what they capture.
public class LocalClosures {
    interface Scale {
        int by(int x);
    }

    private int factor = 3;
    private final StringBuilder log = new StringBuilder();

    int scaled(int[] values) {
        Scale scale = x -> x * factor;
        int total = 0;
        for (int v : values) {
            total += scale.by(v);
        }
        factor++;
        return total + scale.by(1);
    }

    String trace(String[] words) {
        IntPredicate keep = i -> words[i].length() > 2;
        for (int i = 0; i < words.length; i++) {
            if (keep.test(i)) {
                log.append(words[i]).append(' ');
            }
        }
        return log.toString().trim();
    }

    static Scale stored(int k) {
        Scale s;
        {
            Scale t = x -> x + k;
            s = t;
        }
        {
            Scale t = x -> x - k;
            System.out.println("inner " + t.by(10));
        }
        return s;
    }

    public static void main(String[] args) {
        LocalClosures c = new LocalClosures();
        System.out.println(c.scaled(new int[] {1, 2, 3}) + " " + c.factor);
        System.out.println(c.trace(new String[] {"a", "tree", "of", "words"}));
        IntBinaryOperator max = (a, b) -> a > b ? a : b;
        int best = Integer.MIN_VALUE;
        int[] numbers = {4, -2, 9, 7};
        for (int n : numbers) {
            best = max.applyAsInt(best, n);
        }
        System.out.println(best);
        String name = "closure";
        IntPredicate longer = n -> name.length() > n;
        System.out.println(longer.test(3) + " " + longer.test(10) + " " + name);
        System.out.println(stored(5).by(1));
    }
}
