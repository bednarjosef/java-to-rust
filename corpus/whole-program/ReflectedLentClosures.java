import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;

// A reflective call of a method whose closure parameter is lent and may be null, in a program
// where a thread reaches the closure type, which makes it an `Arc` of a `dyn Fn + Send + Sync`:
// the argument that the call casts from its `Object` is lent as an `Option<&dyn Fn>`.
public class ReflectedLentClosures {
    interface Op { int apply(int x); }

    static Op kept;

    static int run(Op op, int x) { return op.apply(x) + 1; }

    public static void main(String[] args) throws Exception {
        int k = args.length + 3;
        kept = x -> x * k;
        int[] out = new int[1];
        Thread t = new Thread(() -> out[0] = run(kept, 5));
        t.start();
        t.join();
        System.out.println("thread " + out[0] + ", main " + run(kept, 2));
        Method run = ReflectedLentClosures.class.getDeclaredMethod("run", Op.class, int.class);
        try {
            System.out.println(run.invoke(null, null, 4));
        } catch (InvocationTargetException e) {
            System.out.println("reflected " + e.getCause().getClass().getSimpleName());
        }
    }
}
