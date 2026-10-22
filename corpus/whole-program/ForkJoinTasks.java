// ForkJoinPool's tasks: a submitted lambda on the common pool, a RecursiveTask that forks and
// joins on a pool of its own, a task whose exception the joining thread gets, and the messages of
// NullPointerExceptions that the JDK's code throws.
import java.util.concurrent.ExecutionException;
import java.util.concurrent.ForkJoinPool;
import java.util.concurrent.RecursiveTask;

public class ForkJoinTasks {
    static class Fibonacci extends RecursiveTask<Integer> {
        final int n;

        Fibonacci(int n) {
            this.n = n;
        }

        @Override
        protected Integer compute() {
            if (n < 2) {
                return n;
            }
            Fibonacci first = new Fibonacci(n - 1);
            first.fork();
            return new Fibonacci(n - 2).compute() + first.join();
        }
    }

    static class Failing extends RecursiveTask<Integer> {
        @Override
        protected Integer compute() {
            throw new IllegalStateException("no value");
        }
    }

    public static void main(String[] args) throws Exception {
        System.out.println(ForkJoinPool.commonPool().submit(() -> 6 * 7).get());
        ForkJoinPool pool = new ForkJoinPool(3);
        System.out.println(pool.invoke(new Fibonacci(15)));
        try {
            pool.invoke(new Failing());
        } catch (IllegalStateException e) {
            System.out.println("joined: " + e.getMessage());
        }
        try {
            pool.submit(new Failing()).get();
        } catch (ExecutionException e) {
            System.out.println("got: " + e.getCause());
        }
        pool.shutdown();
        try {
            new ThreadGroup(null, "orphan");
        } catch (NullPointerException e) {
            System.out.println(e.getMessage());
        }
    }
}
