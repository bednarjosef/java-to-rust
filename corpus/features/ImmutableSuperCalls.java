// Objects that never change after construction call, through `super`, a superclass's method that
// needs the whole object (it calls a method the subclasses override), as the stream library's
// `ReduceOp` subclasses do. A method overrides two methods that erase to one signature, an
// abstract class's and its interface's, so one bridge implements both.
public class ImmutableSuperCalls {
    abstract static class Op {
        final int shape;

        Op(int shape) {
            this.shape = shape;
        }

        abstract int flags();

        int evaluate(int x) {
            return x * flags() + shape;
        }
    }

    static final class Twice extends Op {
        Twice() {
            super(2);
        }

        int flags() {
            return 2;
        }

        @Override
        int evaluate(int x) {
            if (x < 0) {
                return 0;
            }
            return super.evaluate(x) + 1;
        }
    }

    static final class Thrice extends Op {
        Thrice() {
            super(3);
        }

        int flags() {
            return 3;
        }
    }

    interface Task<R> {
        R run(int x);
    }

    abstract static class Named<R> implements Task<R> {
        final String name;

        Named(String name) {
            this.name = name;
        }

        public abstract R run(int x);
    }

    static final class Square extends Named<Integer> {
        Square() {
            super("square");
        }

        @Override
        public Integer run(int x) {
            return x * x;
        }
    }

    public static void main(String[] args) {
        Op[] ops = {new Twice(), new Thrice()};
        for (Op op : ops) {
            System.out.println(op.evaluate(5) + " " + op.evaluate(-1));
        }
        Task<Integer> task = new Square();
        Named<Integer> named = new Square();
        System.out.println(task.run(7) + " " + named.run(3) + " " + named.name);
    }
}
